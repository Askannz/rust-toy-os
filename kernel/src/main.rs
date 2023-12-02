#![no_main]
#![no_std]
#![feature(alloc_error_handler)]
#![feature(abi_x86_interrupt)]

use core::panic::PanicInfo;
use alloc::format;
use alloc::vec::Vec;
use lazy_static::lazy_static;
use uefi::prelude::{entry, Handle, SystemTable, Boot, Status};
use uefi::table::boot::MemoryType;
use smoltcp::wire::{IpAddress, IpCidr};

use applib::{Color, Rect, Framebuffer, SystemState, PointerState, MAX_KEYS_PRESSED, decode_png};
use applib::drawing::text::{DEFAULT_FONT, draw_str};
use applib::drawing::primitives::{draw_rect, blend_rect};

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

use applib::keymap::{EventType, Keycode};
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
    grab_pos: Option<(u32, u32)>,
    time_used: f64,
}

const APPLICATIONS: [AppDescriptor; 3] = [
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
    AppDescriptor {
        data: include_bytes!("../../embedded_data/terminal.wasm"),
        launch_rect: Rect { x0: 100, y0: 200, w: 200, h: 40 },
        name: "Terminal",
        init_win_rect: Rect { x0: 400, y0: 300, w: 400, h: 200 }
    },
];

const FPS_TARGET: f64 = 60.0;

lazy_static! {
    static ref WALLPAPER: Vec<u8> = decode_png(include_bytes!("../../wallpaper.png"));
}

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
    let mut virtio_inputs = [
        VirtioInput::new(&mut pci_devices),
        VirtioInput::new(&mut pci_devices),
    ];
    let virtio_net = VirtioNetwork::new(&mut pci_devices);

    log::info!("All VirtIO devices created");

    virtio_gpu.init_framebuffer();
    virtio_gpu.flush();

    log::info!("Display initialized");

    let (w, h) = virtio_gpu.get_dims();
    let wasm_engine = WasmEngine::new();

    let mut applications: Vec<App> = APPLICATIONS.iter().map(|app_desc| App {
        descriptor: app_desc.clone(),
        wasm_app: wasm_engine.instantiate_app(app_desc.data, app_desc.name, &app_desc.init_win_rect),
        is_open: false,
        rect: app_desc.init_win_rect.clone(),
        grab_pos: None,
        time_used: 0.0,
    }).collect();

    log::info!("Applications loaded");

    let port = 1234;
    let ip_cidr = IpCidr::new(IpAddress::v4(10, 0, 0, 1), 24);
    let mut server = HttpServer::new(virtio_net, ip_cidr, port);

    log::info!("HTTP server initialized");

    let runtime_services = unsafe { system_table.runtime_services() };
    let clock = SystemClock::new(runtime_services);
    let mut fps_manager = FpsManager::new(FPS_TARGET);

    let mut system_state = SystemState {
        pointer: PointerState { x: 0, y: 0, left_clicked: false, right_clicked: false },
        keyboard: [None; MAX_KEYS_PRESSED],
        time: clock.time()
    };

    log::info!("Entering main loop");

    loop {

        fps_manager.start_frame(&clock);

        system_state.time = clock.time();
        update_input_state(&mut system_state, (w as u32, h as u32), &mut virtio_inputs);

        server.update();

        virtio_gpu.framebuffer.copy_from_slice(&WALLPAPER[..]);

        let fb_data = unsafe {
            virtio_gpu.framebuffer.as_mut().align_to_mut::<u32>().1
        };
        let mut framebuffer = Framebuffer::new(fb_data, w, h);

        //log::debug!("{:?}", system_state);

        update_apps(&mut framebuffer, &clock, &system_state, &mut applications);
        draw_cursor(&mut framebuffer, &system_state);

        //applications.iter().for_each(|app| log::debug!("{}: {}ms", app.descriptor.name, app.time_used));

        fps_manager.end_frame(&clock, &mut framebuffer);
        virtio_gpu.flush();
    }


    //loop { x86_64::instructions::hlt(); }

}

fn update_apps(fb: &mut Framebuffer, clock: &SystemClock, system_state: &SystemState, applications: &mut Vec<App>) {

    const ALPHA_SHADOW: u8 = 100;

    const COLOR_IDLE: Color = Color::from_rgba(0x44, 0x44, 0x44, 0xff);
    const COLOR_HOVER: Color = Color::from_rgba(0x88, 0x88, 0x88, 0xff);
    const COLOR_SHADOW: Color = Color::from_rgba(0x0, 0x0, 0x0, ALPHA_SHADOW);
    const COLOR_TEXT: Color = Color::from_rgba(0xff, 0xff, 0xff, 0xff);

    const OFFSET_SHADOW: u32 = 10;
    const TEXT_MARGIN: u32 = 5;
    const DECO_PADDING: u32 = 5;

    for app in applications.iter_mut() {

        let rect = &app.descriptor.launch_rect;

        let pointer_state = &system_state.pointer;
        let launch_hover = rect.check_contains_point(pointer_state.x, pointer_state.y);

        let color = if launch_hover { COLOR_HOVER } else { COLOR_IDLE };

        if launch_hover && pointer_state.left_clicked && !app.is_open {
            log::info!("{} is open", app.descriptor.name);
            app.is_open = true;
        }

        draw_rect(fb, &rect, color);

        let text_x0 = rect.x0 + TEXT_MARGIN;
        let text_y0 = rect.y0 + TEXT_MARGIN;
        draw_str(fb, app.descriptor.name, text_x0, text_y0, &DEFAULT_FONT, COLOR_TEXT);

        if app.is_open {

            let font_h = DEFAULT_FONT.char_h as u32;
            let deco_rect = Rect {
                x0: app.rect.x0 - DECO_PADDING,
                y0: app.rect.y0 - font_h - 2 * DECO_PADDING,
                w: app.rect.w + 2 * DECO_PADDING,
                h: app.rect.h + 3 * DECO_PADDING + font_h,
            };

            if let Some((dx, dy)) = app.grab_pos {
                if pointer_state.left_clicked {
                    app.rect.x0 = pointer_state.x - dx;
                    app.rect.y0 = pointer_state.y - dy;
                } else {
                    app.grab_pos = None
                }
            } else {
                let app_hover = deco_rect.check_contains_point(pointer_state.x, pointer_state.y);
                if app_hover && pointer_state.left_clicked {
                    let dx = pointer_state.x - app.rect.x0;
                    let dy = pointer_state.y - app.rect.y0;
                    app.grab_pos = Some((dx, dy));
                } else if app_hover && pointer_state.right_clicked {
                    app.is_open = false;
                }
            }

            let shadow_rect = Rect { 
                x0: deco_rect.x0 + OFFSET_SHADOW,
                y0: deco_rect.y0 + OFFSET_SHADOW,
                w: deco_rect.w,
                h: deco_rect.h,
            };

            blend_rect(fb, &shadow_rect, COLOR_SHADOW);

            let instance_hover = deco_rect.check_contains_point(pointer_state.x, pointer_state.y);
            let color_app = if instance_hover { COLOR_HOVER } else { COLOR_IDLE };
            draw_rect(fb, &deco_rect, color_app);

            let (x_txt, y_txt) = (app.rect.x0, app.rect.y0 - font_h - DECO_PADDING);
            draw_str(fb, app.descriptor.name, x_txt, y_txt, &DEFAULT_FONT, COLOR_TEXT);

            let mut region = fb.get_region(&app.rect);
            let t0 = clock.time();
            app.wasm_app.step(system_state, &mut region, &app.rect);
            let t1 = clock.time();
            const SMOOTHING: f64 = 0.9;
            app.time_used = (1.0 - SMOOTHING) * (t1 - t0) + SMOOTHING * app.time_used;
        }
    }
}

fn draw_cursor(fb: &mut Framebuffer, system_state: &SystemState) {

    const CURSOR_SIZE: u32 = 5;
    const CURSOR_COLOR: Color = Color::from_rgba(0xff, 0xff, 0xff, 0xff);

    let pointer_state = &system_state.pointer;
    let x = pointer_state.x;
    let y = pointer_state.y;
    draw_rect(fb, &Rect { x0: x, y0: y, w: CURSOR_SIZE, h: CURSOR_SIZE }, CURSOR_COLOR)
}

fn update_input_state(system_state: &mut SystemState, dims: (u32, u32), virtio_inputs: &mut [VirtioInput]) {

    let (w, h) = dims;
    let (w, h) = (w as i32, h as i32);

    for virtio_inp in virtio_inputs.iter_mut() {
        for event in virtio_inp.poll() {

            //log::debug!("{:?}", event);

            match EventType::n(event._type) {

                Some(EventType::EV_SYN) => {},

                Some(EventType::EV_KEY) => match Keycode::n(event.code) {

                    // Mouse click
                    Some(Keycode::BTN_MOUSE_LEFT) => system_state.pointer.left_clicked = event.value == 1,
                    Some(Keycode::BTN_MOUSE_RIGHT) => system_state.pointer.right_clicked = event.value == 1,

                    // Keyboard
                    Some(keycode) => match event.value {

                        // Key was released, freeing its slot
                        0 => system_state.keyboard.iter_mut()
                                .filter(|c| *c == &Some(keycode))
                                .for_each(|c| *c = None),
    
                        // New key pressed, finding a slot for it
                        1 => if !system_state.keyboard.contains(&Some(keycode)) {
                            match system_state.keyboard.iter_mut().find(|c| c.is_none()) {
                                Some(slot) => *slot = Some(keycode),
                                None => log::warn!(
                                    "Dropping keyboard event (all {} slots taken)",
                                    system_state.keyboard.len()
                                )
                            }
                        }
    
                        val => log::warn!("Unknown key state {}", val)
                    },
                    None => log::warn!("Unknown keycode {} for keyboard event", event.code)
                },

                // Mouse movement
                Some(EventType::EV_REL) => match event.code {
                    0 => {  // X axis
                        let dx = event.value as i32;
                        let pointer_state = &mut system_state.pointer;
                        pointer_state.x = i32::max(0, i32::min(w-1, pointer_state.x as i32 + dx)) as u32;
                    }
                    1 => {  // Y axis
                        let dy = event.value as i32;
                        let pointer_state = &mut system_state.pointer;
                        pointer_state.y = i32::max(0, i32::min(h-1, pointer_state.y as i32 + dy)) as u32;
                    },
                    _ => log::warn!("Unknown event code {} for pointer event", event.code)
                },

                _ => log::warn!("Unknown event type {}", event._type)
            };
        }
    }
}

struct FpsManager {
    fps_target: f64,
    frame_start_t: f64,
    frametime: f64,
    used: f64,
}

impl FpsManager {

    fn new(fps_target: f64) -> Self {
        FpsManager { fps_target, frame_start_t: 0.0, frametime: 1000.0 / fps_target, used: 0.0 }
    }

    fn start_frame(&mut self, clock: &SystemClock) {
        self.frame_start_t = clock.time();
    }

    fn end_frame(&mut self, clock: &SystemClock, fb: &mut Framebuffer) {

        const SMOOTHING: f64 = 0.8;

        const WHITE: Color = Color::from_rgba(255, 255, 255, 255);
        const BLACK: Color = Color::from_rgba(0, 0, 0, 255);
        const GREEN: Color = Color::from_rgba(0, 255, 0, 255);
        const RED: Color = Color::from_rgba(255, 0, 0, 255);
        const YELLOW: Color = Color::from_rgba(255, 255, 0, 255);

        let frametime_target = 1000.0 / self.fps_target;

        let fps = 1000.0 / self.frametime;
        let s = format!("{:.2} FPS", fps);
        draw_str(fb, &s, 0, 0, &DEFAULT_FONT, WHITE);

        let char_h = DEFAULT_FONT.char_h as u32;
        let graph_w = 12 * 9;
        let graph_h = 6;
        let used_frac = self.used / frametime_target;
        let used_w = (used_frac * graph_w as f64) as u32;
        let graph_color = {
            if 0.0 <= used_frac && used_frac < 0.50  { GREEN }
            else if 0.50 <= used_frac && used_frac < 0.75  { YELLOW }
            else { RED }
        };
        draw_rect(fb, &Rect { x0: 0, y0: char_h, w: graph_w, h: 12 }, BLACK);
        draw_rect(fb, &Rect { x0: 0, y0: char_h + 3, w: used_w, h: graph_h }, graph_color);

        let available = frametime_target - self.used;
        let budget_color = if available > 0.0 { WHITE } else {  RED };
        let budget_txt = format!("{:>6.2} ms", available);
        draw_str(fb, &budget_txt, 0, char_h + graph_h + 6, &DEFAULT_FONT, budget_color);

        let frame_end_t = clock.time();

        self.used = frame_end_t - self.frame_start_t;

        let new_frametime = match self.used < frametime_target {
            true => {
                clock.spin_delay(frametime_target - self.used);
                frametime_target
            },
            false => self.used
        };

        self.frametime = SMOOTHING * self.frametime + (1.0 - SMOOTHING) * new_frametime;
    }
}

#[panic_handler]
fn panic(info: &PanicInfo) ->  ! {
    log::error!("{}", info);
    loop {}
}
