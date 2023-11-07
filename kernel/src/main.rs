#![no_main]
#![no_std]
#![feature(alloc_error_handler)]
#![feature(abi_x86_interrupt)]

use core::panic::PanicInfo;
use alloc::format;
use alloc::vec::Vec;
use uefi::prelude::{entry, Handle, SystemTable, Boot, Status};
use uefi::table::boot::MemoryType;
use smoltcp::wire::{IpAddress, IpCidr};

use applib::{Color, Rect, Framebuffer, SystemState, PointerState, DEFAULT_FONT, draw_str, draw_rect};

extern crate alloc;

mod memory;
mod serial;
mod logging;
mod time;
mod pci;
mod virtio;
mod smoltcp_virtio;
mod http;

mod wasm;

use time::SystemClock;
use http::HttpServer;


use virtio::gpu::VirtioGPU;
use virtio::input::VirtioInput;
use virtio::network::VirtioNetwork;

use wasm::{WasmEngine, WasmApp};

#[derive(Clone)]
struct AppDescriptor {
    data: &'static [u8],
    launch_rect: Rect,
    name: &'static str,
    init_win_rect: Rect,
}

struct App {
    wasm_app: WasmApp,
    descriptor: AppDescriptor,
    is_open: bool,
    rect: Rect,
    grab_pos: Option<(u32, u32)>
}

const APPLICATIONS: [AppDescriptor; 2] = [
    AppDescriptor {
        data: include_bytes!("../../embedded_data/cube_3d.wasm"),
        launch_rect: Rect { x0: 100, y0: 100, w: 200, h: 40 },
        name: "3D Cube",
        init_win_rect: Rect { x0: 200, y0: 200, w: 200, h: 200 }
    },
    AppDescriptor {
        data: include_bytes!("../../embedded_data/chronometer.wasm"),
        launch_rect: Rect { x0: 100, y0: 150, w: 200, h: 40 },
        name: "Chronometer",
        init_win_rect: Rect { x0: 600, y0: 200, w: 200, h: 200 }
    },
];

const WALLPAPER: &'static [u8] = include_bytes!("../../embedded_data/wallpaper.bin");

static LOGGER: logging::SerialLogger = logging::SerialLogger;
const LOGGING_LEVEL: log::LevelFilter = log::LevelFilter::Debug;

#[entry]
fn main(image: Handle, system_table: SystemTable<Boot>) -> Status {

    log::set_max_level(LOGGING_LEVEL);
    log::set_logger(&LOGGER).unwrap();

    log::info!("Booting kernel");

    let (system_table, memory_map) = system_table
        .exit_boot_services(MemoryType::LOADER_DATA);

    log::info!("Exited UEFI boot services");

    memory::init_allocator(&memory_map);
    memory::init_mapper();

    let mut pci_devices = pci::enumerate();

    let mut virtio_gpu = VirtioGPU::new(&mut pci_devices);
    let mut virtio_input = VirtioInput::new(&mut pci_devices);
    let virtio_net = VirtioNetwork::new(&mut pci_devices);

    log::info!("All VirtIO devices created");

    virtio_gpu.init_framebuffer();
    virtio_gpu.flush();

    log::info!("Display initialized");

    let (w, h) = virtio_gpu.get_dims();
    let mut pointer_state = PointerState { x: 0, y: 0, clicked: false };
    let wasm_engine = WasmEngine::new();

    let mut applications: Vec<App> = APPLICATIONS.iter().map(|app_desc| App {
        descriptor: app_desc.clone(),
        wasm_app: wasm_engine.instantiate_app(app_desc.data),
        is_open: false,
        rect: app_desc.init_win_rect.clone(),
        grab_pos: None
    }).collect();

    log::info!("Applications loaded");

    let port = 1234;
    let ip_cidr = IpCidr::new(IpAddress::v4(10, 0, 0, 1), 24);
    let mut server = HttpServer::new(virtio_net, ip_cidr, port);

    log::info!("HTTP server initialized");

    let runtime_services = unsafe { system_table.runtime_services() };
    let clock = SystemClock::new(runtime_services);
    let mut fps_overlay = FpsOverlay::new(clock.time());

    log::info!("Entering main loop");

    loop {

        pointer_state = update_pointer(&mut virtio_input, (w as u32, h as u32), pointer_state);

        server.update();

        virtio_gpu.framebuffer.copy_from_slice(&WALLPAPER[..]);

        let mut framebuffer = Framebuffer::new(virtio_gpu.framebuffer.as_mut(), w, h);

        let system_state = SystemState {
            pointer: pointer_state.clone(),
            time: clock.time()
        };

        //log::debug!("{:?}", system_state);

        update_apps(&mut framebuffer, &system_state, &mut applications);
        draw_cursor(&mut framebuffer, &system_state);
        fps_overlay.update(clock.time());
        fps_overlay.draw(&mut framebuffer);
        virtio_gpu.flush();
    }


    //loop { x86_64::instructions::hlt(); }

}

fn update_apps(fb: &mut Framebuffer, system_state: &SystemState, applications: &mut Vec<App>) {

    const COLOR_IDLE: Color = Color(0x44, 0x44, 0x44);
    const COLOR_HOVER: Color = Color(0x88, 0x88, 0x88);
    const TEXT_MARGIN: u32 = 5;

    for app in applications.iter_mut() {

        let rect = &app.descriptor.launch_rect;

        let pointer_state = &system_state.pointer;
        let hover = rect.check_contains_point(pointer_state.x, pointer_state.y);

        let color = if hover { &COLOR_HOVER } else { &COLOR_IDLE };

        if hover && pointer_state.clicked && !app.is_open {
            log::info!("{} is open", app.descriptor.name);
            app.is_open = true;
        }

        draw_rect(fb, &rect, color, 255);

        let text_x0 = rect.x0 + TEXT_MARGIN;
        let text_y0 = rect.y0 + TEXT_MARGIN;
        draw_str(fb, app.descriptor.name, text_x0, text_y0, &DEFAULT_FONT, &Color(0xff, 0xff, 0xff));

        if app.is_open {

            let deco_rect = Rect {
                x0: app.rect.x0 - 5,
                y0: app.rect.y0 - 35,
                w: app.rect.w + 2 * 5,
                h: app.rect.h + 2 * 5 + 30,
            };

            if let Some((dx, dy)) = app.grab_pos {
                if pointer_state.clicked {
                    app.rect.x0 = pointer_state.x - dx;
                    app.rect.y0 = pointer_state.y - dy;
                } else {
                    app.grab_pos = None
                }
            } else {
                if pointer_state.clicked && deco_rect.check_contains_point(pointer_state.x, pointer_state.y){
                    let dx = pointer_state.x - app.rect.x0;
                    let dy = pointer_state.y - app.rect.y0;
                    app.grab_pos = Some((dx, dy));
                }
            }

            draw_rect(fb, &deco_rect, &Color(0x88, 0x88, 0x88), 127);
            //draw_rect(fb, &app.rect, &Color(0x00, 0x00, 0x00), 0.5);
            draw_str(fb, app.descriptor.name, app.rect.x0, app.rect.y0 - 30, &DEFAULT_FONT, &Color(0xff, 0xff, 0xff));

            let mut region = fb.get_region(&app.rect);
            app.wasm_app.step(system_state, &mut region);
        }
    }
}

fn draw_cursor(fb: &mut Framebuffer, system_state: &SystemState) {
    let pointer_state = &system_state.pointer;
    let x = pointer_state.x;
    let y = pointer_state.y;
    draw_rect(fb, &Rect { x0: x, y0: y, w: 5, h: 5 }, &Color(0xff, 0xff, 0xff), 255)
}

fn update_pointer(virtio_input: &mut VirtioInput, dims: (u32, u32), status: PointerState) -> PointerState {

    let (w, h) = dims;
    let (w, h) = (w as i32, h as i32);

    let mut status = status;

    for event in virtio_input.poll() {
        if event._type == 0x2 {
            if event.code == 0 {  // X axis
                let dx = event.value as i32;
                status.x = i32::max(0, i32::min(w-1, status.x as i32 + dx)) as u32;
            } else {  // Y axis
                let dy = event.value as i32;
                status.y = i32::max(0, i32::min(h-1, status.y as i32 + dy)) as u32;
            }
        } else if event._type == 0x1 {
            status.clicked = event.value == 1
        }
        //log::debug!("{:?}", status);
    }

    status
}

struct FpsOverlay {
    last_t: u64,
    frametime: f64
}

impl FpsOverlay {
    fn new(t: u64) -> Self {
        FpsOverlay { last_t: t, frametime: 0.0 }
    }
    fn update(&mut self, now: u64) {
        const SMOOTHING: f64 = 0.9;
        let new_frametime = (now - self.last_t) as f64;
        self.last_t = now;
        self.frametime = SMOOTHING * self.frametime + (1.0 - SMOOTHING) * new_frametime; 
    }
    fn draw(&self, fb: &mut Framebuffer) {
        let fps = 1000.0 / self.frametime;
        let s = format!("{:.2} FPS", fps);
        draw_str(fb, &s, 0, 0, &DEFAULT_FONT, &Color(255, 255, 255));
    }
}


#[panic_handler]
fn panic(info: &PanicInfo) ->  ! {
    log::error!("{}", info);
    loop {}
}
