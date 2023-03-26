#![no_main]
#![no_std]
#![feature(alloc_error_handler)]
#![feature(abi_x86_interrupt)]

use core::panic::PanicInfo;
use alloc::vec::Vec;
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

struct Application {
    data: &'static [u8],
    entrypoint: u64,
    launcher: Rect
}

const APPLICATIONS: [Application; 1] = [
    Application {
        data: include_bytes!("pedump"),
        entrypoint: 0x1000,
        launcher: Rect { x0: 10, y0: 10, w: 40, h: 40 }
    }
];

struct MouseStatus {
    x: i32,
    y: i32,
    clicked: bool
}

const FONT_BYTES: &'static [u8] = include_bytes!("font.bin");
const FONT_NB_CHARS: usize = 95;
const FONT_CHAR_H: usize = 64;
const FONT_CHAR_W: usize = 32;

const WALLPAPER: &'static [u8] = include_bytes!("wallpaper.bin");

const BOOT_INFO: &'static BootInfo  = &BootInfo { physical_memory_offset: 0 };


// ---- SHARED ----

#[derive(Clone)]
struct Color(u8, u8, u8);
#[derive(Clone)]
struct Rect { x0: i32, y0: i32, w: i32, h: i32 }

impl Rect {
    fn check_in(&self, x: i32, y: i32) -> bool {
        return 
            x >= self.x0 && x < self.x0 + self.w &&
            y >= self.y0 && y < self.y0 + self.h
    }
}

pub struct Framebuffer<'a> {
    data: &'a mut [u8],
    w: i32,
    h: i32,
}

pub struct FrameBufSlice<'a, 'b> {
    fb: &'b mut Framebuffer<'a>,
    rect: Rect
}

impl<'a, 'b> FrameBufSlice<'a, 'b> {
    fn set_pixel(&mut self, x: i32, y: i32, color: &Color) {
        let Color(r, g, b) = *color;
        let i = (((y+self.rect.y0) * self.fb.w + x + self.rect.x0) * 4) as usize;
        self.fb.data[i] = r;
        self.fb.data[i+1] = g;
        self.fb.data[i+2] = b;
        self.fb.data[i+3] = 0xff;
    }
}

#[repr(C)]
pub struct Oshandle<'a, 'b> {
    fb: FrameBufSlice<'a, 'b>,
    cursor_x: i32,
    cursor_y: i32,
}

// ---- END SHARED ----

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
    let mut mouse_status = MouseStatus { x: 0, y: 0, clicked: false };
    let mut active_apps = [false; APPLICATIONS.len()];

    loop {

        mouse_status = update_cursor(&mut virtio_input, (w, h), mouse_status);

        virtio_gpu.framebuffer.copy_from_slice(&WALLPAPER[..]);

        let mut framebuffer = Framebuffer { data: &mut virtio_gpu.framebuffer[..], w, h };
        let framebuf_slice = FrameBufSlice {
            fb: &mut framebuffer,
            rect: Rect { x0: 100, y0: 100, w: 200, h: 200 }
        };

        //draw_icons(&mut framebuffer, &mouse_status);

        let mut handle = Oshandle {
            fb: framebuf_slice,
            cursor_x: mouse_status.x, cursor_y: mouse_status.y
        };
        call_app(&mut handle, &APPLICATIONS[0]);
        //draw_str(&mut handle.fb, 20, 20, "Babouya", &Color(0xff, 0x0, 0x0));

        set_pixel(&mut virtio_gpu, (mouse_status.x, mouse_status.y));
        virtio_gpu.flush();
    }


    //loop { x86_64::instructions::hlt(); }

}

fn draw_icons(fb: &mut Framebuffer, mouse_status: &MouseStatus) {

    let COLOR_IDLE = Color(0x44, 0x44, 0x44);
    let COLOR_HOVER = Color(0x88, 0x88, 0x88);

    for app in APPLICATIONS {

        let color = match app.launcher.check_in(mouse_status.x, mouse_status.y) {
            true => &COLOR_HOVER,
            false => &COLOR_IDLE
        };

        draw_rect(fb, &app.launcher, color);
    }
}

fn update_cursor(virtio_input: &mut VirtioInput, dims: (i32, i32), status: MouseStatus) -> MouseStatus {

    let (w, h) = dims;

    let mut status = status;

    for event in virtio_input.poll() {
        if event._type == 0x2 {
            if event.code == 0 {  // X axis
                let dx = event.value as i32;
                status.x = i32::max(0, i32::min(w-1, status.x + dx));
            } else {  // Y axis
                let dy = event.value as i32;
                status.y = i32::max(0, i32::min(h-1, status.y + dy));
            }
        } else if event._type == 0x1 {
            status.clicked = event.value == 1
        }
    }

    status
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


fn draw_rect(fb: &mut Framebuffer, rect: &Rect, color: &Color) {
    let Color(r, g, b) = *color;
    for x in rect.x0..rect.x0+rect.w {
        for y in rect.y0..rect.y0+rect.h {
            let i = ((y * fb.w + x) * 4) as usize;
            fb.data[i] = r;
            fb.data[i+1] = g;
            fb.data[i+2] = b;
            fb.data[i+3] = 0xff;
        }
    }
}


fn draw_str(fb: &mut Framebuffer, x0: i32, y0: i32, s: &str, color: &Color) {
    let mut x = x0;
    for c in s.as_bytes() {
        draw_char(fb, x, y0, *c, color);
        x += FONT_CHAR_W as i32;
    }
}

fn draw_char(fb: &mut Framebuffer, x0: i32, y0: i32, c: u8, color: &Color) {

    assert!(c >= 32 && c <= 126);

    let c_index = (c - 32) as i32;
    let Color(r, g, b) = *color;
    let cw = FONT_CHAR_W as i32;
    let ch = FONT_CHAR_H as i32;
    let n_chars = FONT_NB_CHARS as i32;

    for x in 0..cw {
        for y in 0..ch {
            let i_font = (y * cw * n_chars + x + c_index * cw) as usize;
            if FONT_BYTES[i_font] > 0 {
                let i = (((y0 + y) * fb.w + x + x0) * 4) as usize;
                fb.data[i]   = r;
                fb.data[i+1] = g;
                fb.data[i+2] = b;
                fb.data[i+3] = 0xff;
            }
        }
    }

}

fn call_app(handle: &mut Oshandle, app: &Application) -> () {

    let code_ptr =  app.data.as_ptr();
    let entrypoint_ptr = unsafe { code_ptr.offset(app.entrypoint as isize)};

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
