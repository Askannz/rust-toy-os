#![no_main]
#![no_std]
#![feature(alloc_error_handler)]
#![feature(abi_x86_interrupt)]

use core::panic::PanicInfo;
use uefi::prelude::*;
use uefi::table::boot::{MemoryDescriptor, MemoryType};
use linked_list_allocator::LockedHeap;
use x86_64::VirtAddr;
use x86_64::structures::paging::{PageTable, OffsetPageTable, Translate, mapper::TranslateResult};

extern crate alloc;

mod serial;
mod pci;
mod virtio;
//mod interrupts;

use virtio::gpu::VirtioGPU;
use virtio::input::{VirtioInput};
use virtio::{VirtioDevice, BootInfo};

#[global_allocator]
static ALLOCATOR: LockedHeap = LockedHeap::empty();

const TEST_CODE: &'static [u8] = include_bytes!("pedump");
//const START_POINT: u64 = 0x201120;
const START_POINT: u64 = 0x1000;

const BOOT_INFO: &'static BootInfo  = &BootInfo { physical_memory_offset: 0 };

pub struct Framebuffer<'a> {
    data: &'a mut [u8],
    w: i32,
    h: i32,
}

#[repr(C)]
pub struct Oshandle<'a> {
    fb: Framebuffer<'a>,
    cursor_x: i32,
    cursor_y: i32,
}

#[entry]
fn main(image: Handle, mut system_table: SystemTable<Boot>) -> Status {

    system_table.stdout().reset(false).unwrap();

    x86_64::instructions::interrupts::enable();

    let mmap_storage = {
        let base_size = system_table.boot_services().memory_map_size().map_size;
        let extra = 8 * core::mem::size_of::<MemoryDescriptor>();
        let max_mmap_size = base_size + extra;
        let ptr = system_table
            .boot_services()
            .allocate_pool(MemoryType::LOADER_DATA, max_mmap_size)
            .unwrap();
        unsafe { core::slice::from_raw_parts_mut(ptr, max_mmap_size) }
    };

    let (system_table, mut memory_map) = system_table
        .exit_boot_services(image, mmap_storage)
        .expect("Failed to exit boot services");

    let desc = memory_map
        .filter(|desc| desc.ty == MemoryType::CONVENTIONAL)
        .max_by_key(|desc| desc.page_count)
        .unwrap();
    serial_println!("{:?}", desc);

    const HEAP_SIZE: usize = 10000 * 4 * 1024;
    assert!(HEAP_SIZE < (desc.page_count * 4000) as usize);
    unsafe {
        ALLOCATOR.lock().init(desc.phys_start as usize, HEAP_SIZE);
    }


    let mapper = unsafe { 

        let phys_offset = VirtAddr::new(0x0);

        // Get active L4 table
        let l4_table = {
            use x86_64::registers::control::Cr3;
            let (l4_frame, _) = Cr3::read();

            let phys = l4_frame.start_address();
            let virt = phys_offset + phys.as_u64();
            let ptr: *mut PageTable = virt.as_mut_ptr();
        
            &mut *ptr
        };

        OffsetPageTable::new(l4_table, phys_offset)
    };

    let mut virtio_gpu = {

        let virtio_pci_dev = pci::enumerate()
            .find(|dev| dev.vendor_id == 0x1af4 && dev.device_id == 0x1050)
            .expect("Cannot find VirtIO GPU device");

        let virtio_dev = VirtioDevice::new(BOOT_INFO, virtio_pci_dev);

        VirtioGPU::new(BOOT_INFO, &mapper, virtio_dev)
    };
    

    let mut virtio_input = {

        let virtio_pci_dev = pci::enumerate()
            .find(|dev| dev.vendor_id == 0x1af4 && dev.device_id == 0x1052)
            .expect("Cannot find VirtIO input device");

        let virtio_dev = VirtioDevice::new(BOOT_INFO, virtio_pci_dev);

        VirtioInput::new(BOOT_INFO, &mapper, virtio_dev)
    };

    virtio_gpu.init_framebuffer(&mapper);
    virtio_gpu.flush();

    let (w, h) = virtio_gpu.get_dims();
    let (w, h) = (w as i32, h as i32);
    let (mut x, mut y) = (0, 0);

    loop {

        (x, y) = update_cursor(&mut virtio_input, (w, h), (x, y));

        virtio_gpu.framebuffer.fill(0x00);

        let mut handle = Oshandle {
            fb: Framebuffer { data: &mut virtio_gpu.framebuffer[..], w, h },
            cursor_x: x, cursor_y: y
        };
        call_executable(&mut handle);

        set_pixel(&mut virtio_gpu, (x, y));
        virtio_gpu.flush();
    }


    //loop { x86_64::instructions::hlt(); }

}

fn update_cursor(virtio_input: &mut VirtioInput, dims: (i32, i32), pos: (i32, i32)) -> (i32, i32) {

    let (w, h) = dims;
    let (mut x, mut y) = pos;

    for event in virtio_input.poll() {
        if event._type == 0x2 {
            if event.code == 0 {  // X axis
                let dx = event.value as i32;
                x = i32::max(0, i32::min(w-1, x + dx));
            } else {  // Y axis
                let dy = event.value as i32;
                y = i32::max(0, i32::min(h-1, y + dy));
            }
        }
    }

    (x, y)
}

fn set_pixel(virtio_gpu: &mut VirtioGPU, pos: (i32, i32)) {

    let (w, h) = virtio_gpu.get_dims();
    let (w, h) = (w as i32, h as i32);
    let (x, y) = pos;

    let i = ((y * w + x) * 4) as usize;
    let fb = &mut virtio_gpu.framebuffer;

    fb[i] = 0xff;
    fb[i+1] = 0xff;
    fb[i+2] = 0xff;
    fb[i+3] = 0xff;

}

fn call_executable(handle: &mut Oshandle) -> () {

    let code_ptr =  TEST_CODE.as_ptr();
    let entrypoint_ptr = unsafe { code_ptr.offset(START_POINT as isize)};

    let exec_data: extern "C" fn (&mut Oshandle) = unsafe {  
        core::mem::transmute(entrypoint_ptr)
    };

    exec_data(handle);
}


#[panic_handler]
fn panic(info: &PanicInfo) ->  ! {
    serial_println!("{}", info);
    loop {}
}

fn get_phys_addr<T>(mapper: &impl Translate, p: *mut T) -> u64 {

    let virt_addr = VirtAddr::new(p as u64);
    let (frame, offset) = match mapper.translate(virt_addr) {
        TranslateResult::Mapped { frame, offset, .. } => (frame, offset),
        v => panic!("Cannot translate page: {:?}", v)
    };

    (frame.start_address() + offset).as_u64()
}
