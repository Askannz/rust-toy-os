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

use applib::{Color, Rect, Framebuffer, FrameBufSlice, Oshandle};

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

#[derive(Clone)]
struct AppDescriptor {
    data: &'static [u8],
    entrypoint: u64,
    launch_rect: Rect,
    name: &'static str,
    init_win_rect: Rect,
}

struct App {
    descriptor: AppDescriptor,
    is_open: bool,
    rect: Rect,
    grab_pos: Option<(i32, i32)>
}

const APPLICATIONS: [AppDescriptor; 2] = [
    AppDescriptor {
        data: include_bytes!("../../embedded_data/apps/cube_3d"),
        entrypoint: 0x1000,
        launch_rect: Rect { x0: 100, y0: 100, w: 200, h: 40 },
        name: "3D Cube",
        init_win_rect: Rect { x0: 200, y0: 200, w: 400, h: 400 }
    },
    AppDescriptor {
        data: include_bytes!("../../embedded_data/apps/chronometer"),
        entrypoint: 0x1000,
        launch_rect: Rect { x0: 100, y0: 150, w: 200, h: 40 },
        name: "Chronometer",
        init_win_rect: Rect { x0: 600, y0: 200, w: 200, h: 200 }
    },
];

struct MouseStatus {
    x: i32,
    y: i32,
    clicked: bool
}

const FONT_BYTES: &'static [u8] = include_bytes!("../../embedded_data/fontmap.bin");
const FONT_NB_CHARS: usize = 95;
const FONT_CHAR_H: usize = 24;
const FONT_CHAR_W: usize = 12;

const WALLPAPER: &'static [u8] = include_bytes!("../../embedded_data/wallpaper.bin");

const BOOT_INFO: &'static BootInfo  = &BootInfo { physical_memory_offset: 0 };

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
    let mut applications: Vec<App> = APPLICATIONS.iter().map(|app_desc| App {
        descriptor: app_desc.clone(),
        is_open: false,
        rect: app_desc.init_win_rect.clone(),
        grab_pos: None
    }).collect();

    loop {

        mouse_status = update_mouse(&mut virtio_input, (w, h), mouse_status);

        virtio_gpu.framebuffer.copy_from_slice(&WALLPAPER[..]);

        let mut framebuffer = Framebuffer { data: &mut virtio_gpu.framebuffer[..], w, h };

        update_apps(&mut framebuffer, &mouse_status, &mut applications);

        draw_cursor(&mut framebuffer, &mouse_status);
        virtio_gpu.flush();
    }


    //loop { x86_64::instructions::hlt(); }

}

fn update_apps(fb: &mut Framebuffer, mouse_status: &MouseStatus, applications: &mut Vec<App>) {

    const COLOR_IDLE: Color = Color(0x44, 0x44, 0x44);
    const COLOR_HOVER: Color = Color(0x88, 0x88, 0x88);
    const TEXT_MARGIN: i32 = 5;

    for app in applications.iter_mut() {

        let rect = &app.descriptor.launch_rect;

        let hover = rect.check_in(mouse_status.x, mouse_status.y);

        let color = if hover { &COLOR_HOVER } else { &COLOR_IDLE };

        if hover && mouse_status.clicked && !app.is_open {
            serial_println!("{} is open", app.descriptor.name);
            app.is_open = true;
        }

        draw_rect(fb, &rect, color, 1.0);

        let text_x0 = rect.x0 + TEXT_MARGIN;
        let text_y0 = rect.y0 + TEXT_MARGIN;
        draw_str(fb, text_x0, text_y0, app.descriptor.name, &Color(0xff, 0xff, 0xff));

        if app.is_open {

            let deco_rect = Rect {
                x0: app.rect.x0 - 5,
                y0: app.rect.y0 - 35,
                w: app.rect.w + 2 * 5,
                h: app.rect.h + 2 * 5 + 30,
            };

            if let Some((dx, dy)) = app.grab_pos {
                if mouse_status.clicked {
                    app.rect.x0 = mouse_status.x - dx;
                    app.rect.y0 = mouse_status.y - dy;
                } else {
                    app.grab_pos = None
                }
            } else {
                if mouse_status.clicked && deco_rect.check_in(mouse_status.x, mouse_status.y){
                    let dx = mouse_status.x - app.rect.x0;
                    let dy = mouse_status.y - app.rect.y0;
                    app.grab_pos = Some((dx, dy));
                }
            }

            draw_rect(fb, &deco_rect, &Color(0x88, 0x88, 0x88), 0.5);
            draw_rect(fb, &app.rect, &Color(0x00, 0x00, 0x00), 0.5);
            draw_str(fb, app.rect.x0, app.rect.y0 - 30, app.descriptor.name, &Color(0xff, 0xff, 0xff));

            let mut handle = Oshandle {
                fb: FrameBufSlice { fb, rect: app.rect.clone() },
                cursor_x: mouse_status.x - app.rect.x0,
                cursor_y: mouse_status.y - app.rect.y0
            };

            call_app(&mut handle, &app.descriptor);
        }
    }
}

fn draw_cursor(fb: &mut Framebuffer, mouse_status: &MouseStatus) {
    let x = mouse_status.x;
    let y = mouse_status.y;
    draw_rect(fb, &Rect { x0: x, y0: y, w: 5, h: 5 }, &Color(0xff, 0xff, 0xff), 1.0)
}

fn update_mouse(virtio_input: &mut VirtioInput, dims: (i32, i32), status: MouseStatus) -> MouseStatus {

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


fn draw_rect(fb: &mut Framebuffer, rect: &Rect, color: &Color, alpha: f32) {

    let x0 = i32::max(0, rect.x0);
    let x1 = i32::min(fb.w-1, rect.x0+rect.w);
    let y0 = i32::max(0, rect.y0);
    let y1 = i32::min(fb.h-1, rect.y0+rect.h);

    let Color(r, g, b) = *color;
    for x in x0..=x1 {
        for y in y0..=y1 {
            let i = ((y * fb.w + x) * 4) as usize;
            fb.data[i] = blend(fb.data[i], r, alpha);
            fb.data[i+1] = blend(fb.data[i], g, alpha);
            fb.data[i+2] = blend(fb.data[i], b, alpha);
            fb.data[i+3] = 0xff;
        }
    }
}

fn clamp_in_rect(rect: &Rect, x: i32, y: i32) -> (i32, i32) {
    let x = i32::min(rect.x0+rect.w-1, i32::max(0, x));
    let y = i32::min(rect.y0+rect.h-1, i32::max(0, y));
    (x, y)
}

fn blend(a: u8, b: u8, alpha: f32) -> u8 {
    ((a as f32) * (1.0 - alpha) + (b as f32) * alpha) as u8
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

fn call_app(handle: &mut Oshandle, app: &AppDescriptor) -> () {

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
