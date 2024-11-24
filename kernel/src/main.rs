#![no_main]
#![no_std]
#![feature(alloc_error_handler)]
#![feature(abi_x86_interrupt)]

use alloc::vec::Vec;
use alloc::format;
use core::panic::PanicInfo;
use num_traits::Float;
use rand::rngs::SmallRng;
use rand::SeedableRng;
use uefi::prelude::{entry, Boot, Handle, Status, SystemTable};
use uefi::table::boot::MemoryType;

use applib::drawing::primitives::draw_rect;
use applib::drawing::text::{draw_str};
use applib::input::{InputEvent, InputState};
use applib::uitk::{self, UiContext};
use applib::{BorrowedMutPixels, Color, FbView, FbViewMut, Framebuffer, OwnedPixels, Rect};

extern crate alloc;

mod app;
mod logging;
mod memory;
mod network;
mod pci;
mod resources;
mod serial;
mod shell;
mod system;
mod time;
mod virtio;
mod wasm;
mod topbar;
mod allocator;
mod stats;

use time::SystemClock;

use virtio::gpu::VirtioGPU;
use virtio::input::VirtioInput;
use virtio::network::VirtioNetwork;

use app::{run_apps, App, AppsInteractionState, AppsManager, AppState};
use applib::input::keymap::{EventType, Keycode};
use resources::{APPLICATIONS, WALLPAPER, STYLESHEET};
use system::System;
use wasm::WasmEngine;

const FPS_TARGET: f64 = 60.0;
const LIMIT_FPS: bool = true;

static LOGGER: logging::SerialLogger = logging::SerialLogger;
const LOGGING_LEVEL: log::LevelFilter = log::LevelFilter::Debug;

#[entry]
fn main(image: Handle, system_table: SystemTable<Boot>) -> Status {
    log::set_max_level(LOGGING_LEVEL);
    log::set_logger(&LOGGER).unwrap();

    log::info!("Booting kernel");

    let (system_table, memory_map) = system_table.exit_boot_services(MemoryType::LOADER_DATA);

    log::info!("Exited UEFI boot services");

    memory::init_mapper();
    memory::init_allocator(&memory_map);

    let mut pci_devices = pci::enumerate();

    let mut virtio_gpu = VirtioGPU::new(&mut pci_devices);
    let mut virtio_inputs = [
        VirtioInput::new(&mut pci_devices),
        VirtioInput::new(&mut pci_devices),
    ];
    let virtio_net = VirtioNetwork::new(&mut pci_devices);

    log::info!("All VirtIO devices created");

    let runtime_services = unsafe { system_table.runtime_services() };
    let clock = SystemClock::new(runtime_services);

    log::info!("System clock initialized");

    virtio_gpu.init_framebuffer();
    virtio_gpu.flush();

    log::info!("Display initialized");

    let tcp_stack = network::TcpStack::new(&clock, virtio_net);

    //let socket_handle = tcp_stack.borrow_mut().connect(Ipv4Address([93, 184, 216, 34]), 80);

    log::info!("TCP stack initialized");

    let (w, h) = virtio_gpu.get_dims();
    let (w, h) = (w as u32, h as u32);
    let wasm_engine = WasmEngine::new();

    let mut input_state = InputState::new(w, h);

    let app_names: Vec<&str> = APPLICATIONS.iter().map(|desc| desc.name).collect();
    let heap_size = memory::ALLOCATOR.size();
    let system_stats = stats::SystemStats::new(heap_size, &app_names);

    let mut system = System {
        clock,
        tcp_stack,
        rng: SmallRng::seed_from_u64(0),
        stylesheet: &STYLESHEET,
        stats: system_stats,
    };

    let apps: Vec<App> = APPLICATIONS
        .iter()
        .map(|app_desc| App {
            descriptor: app_desc.clone(),
            app_state: AppState::Init,
            is_open: false,
            rect: app_desc.init_win_rect.clone(),
            time_used: 0.0,
        })
        .collect();

    let mut apps_manager = AppsManager::new(apps);

    log::info!("Applications loaded");

    let mut fps_manager = FpsManager::new(FPS_TARGET);

    let mut ui_store = uitk::UiStore::new();
    let mut uuid_provider = uitk::UuidProvider::new();

    let mut apps_interaction_state = AppsInteractionState::Idle;

    log::info!("Entering main loop");

    loop {

        let t0 = system.clock.time();

        {
            let System {
                clock, tcp_stack, ..
            } = &mut system;
            fps_manager.start_frame(clock);
            tcp_stack.poll_interface(clock);
        }

        let time = system.clock.time();

        let datetime = SystemClock::utc_datetime(runtime_services);

        update_input_state(&mut input_state, (w, h), &mut virtio_inputs);

        let mut framebuffer = Framebuffer::<BorrowedMutPixels>::from_bytes(&mut virtio_gpu.framebuffer, w, h);

        let wallpaper: &Framebuffer<OwnedPixels> = &WALLPAPER;
        framebuffer.copy_from_fb(wallpaper, (0, 0), false);

        //log::debug!("{:?}", system_state);

        let mut uitk_context = ui_store.get_context(
            &mut framebuffer,
            &system.stylesheet,
            &input_state,
            &mut uuid_provider,
            time
        );

        topbar::topbar(&mut uitk_context, datetime);

        run_apps(
            &mut uitk_context,
            &mut system,
            &wasm_engine,
            &mut apps_manager,
            &input_state,
            &mut apps_interaction_state,
        );

        //applications.iter().for_each(|app| log::debug!("{}: {}ms", app.descriptor.name, app.time_used));

        draw_cursor(uitk_context.fb, &input_state);

        {
            let System { clock, .. } = &mut system;
            fps_manager.end_frame(&mut uitk_context, clock);
        }

        virtio_gpu.flush();

        let (net_recv, net_sent) = system.tcp_stack.pop_counters();

        let t1 = system.clock.time();

        *system.stats.get_system_point_mut() = stats::SystemDataPoint {
            heap_usage: memory::ALLOCATOR.get_usage(),
            frametime_used: t1 - t0,
            net_recv,
            net_sent,
        };

        // DEBUG
        // let system_stats = system.stats.get_system_point_mut();
        // log::debug!("{:?}", system_stats);
        // for app_desc in APPLICATIONS.iter() {
        //     let app_stats = system.stats.get_app_point_mut(&app_desc.name);
        //     log::debug!("{}: {:?}", app_desc.name, app_stats);
        // }
        let net_recv: usize = system.stats.get_system_history().map(|data_point| data_point.net_recv).sum();
        log::debug!("Net recv: {} kB", net_recv / 1_000);

        system.stats.next_frame();

        // let (heap_used, heap_total) = memory::ALLOCATOR.get_stats();
        // log::debug!("Memory: {}/{} MB", heap_used / 1_000_000, heap_total / 1_000_000);
        // let (recv_counter, sent_counter) = system.tcp_stack.get_counters();
        // recv_counter_total += recv_counter;
        // sent_counter_total += sent_counter;
        // log::debug!("Network: {} kB down / {} kB up", recv_counter_total / 1_000, sent_counter_total / 1_000);
    }

    //loop { x86_64::instructions::hlt(); }
}

fn draw_cursor<F: FbViewMut>(fb: &mut F, input_state: &InputState) {
    const SIZE: u32 = 5;
    const BORDER: u32 = 1;

    let pointer_state = &input_state.pointer;
    let x = pointer_state.x;
    let y = pointer_state.y;

    let rect_outer = Rect {
        x0: x,
        y0: y,
        w: SIZE,
        h: SIZE,
    };
    let rect_inner = Rect {
        x0: x + BORDER as i64,
        y0: y + BORDER as i64,
        w: SIZE - 2 * BORDER,
        h: SIZE - 2 * BORDER,
    };

    draw_rect(fb, &rect_outer, Color::BLACK, false);
    draw_rect(fb, &rect_inner, Color::WHITE, false);
}

fn update_input_state(
    input_state: &mut InputState,
    dims: (u32, u32),
    virtio_inputs: &mut [VirtioInput],
) {
    let (w, h) = dims;
    let (w, h) = (w as i32, h as i32);

    input_state.clear_events();
    input_state.pointer.left_click_trigger = false;
    input_state.pointer.right_click_trigger = false;
    input_state.pointer.delta_x = 0;
    input_state.pointer.delta_y = 0;

    for virtio_inp in virtio_inputs.iter_mut() {
        for event in virtio_inp.poll() {
            //log::debug!("{:?}", event);

            match EventType::n(event._type) {
                Some(EventType::EV_SYN) => {}

                Some(EventType::EV_KEY) => match Keycode::n(event.code) {
                    // Mouse click
                    Some(Keycode::BTN_MOUSE_LEFT) => match event.value {
                        1 => {
                            if !input_state.pointer.left_clicked {
                                input_state.pointer.left_click_trigger = true;
                            }
                            input_state.pointer.left_clicked = true;
                        }
                        _ => input_state.pointer.left_clicked = false,
                    },
                    Some(Keycode::BTN_MOUSE_RIGHT) => match event.value {
                        1 => {
                            if !input_state.pointer.right_clicked {
                                input_state.pointer.right_click_trigger = true;
                            }
                            input_state.pointer.right_clicked = true;
                        }
                        _ => input_state.pointer.right_clicked = false,
                    },

                    // Keyboard
                    Some(keycode) => match event.value {
                        0 => input_state.add_event(InputEvent::KeyRelease { keycode }),
                        1 => input_state.add_event(InputEvent::KeyPress { keycode }),
                        val => log::warn!("Unknown key state {}", val),
                    },
                    None => log::warn!("Unknown keycode {} for keyboard event", event.code),
                },

                // Mouse movement
                Some(EventType::EV_REL) => match event.code {
                    0 => {
                        // X axis
                        let dx = (event.value as i32) as i64;
                        let pointer_state = &mut input_state.pointer;
                        let new_x =
                            i64::max(0, i64::min(w as i64 - 1, pointer_state.x as i64 + dx));
                        pointer_state.delta_x += dx;
                        pointer_state.x = new_x;
                    }
                    1 => {
                        // Y axis
                        let dy = (event.value as i32) as i64;
                        let pointer_state = &mut input_state.pointer;
                        let new_y =
                            i64::max(0, i64::min(h as i64 - 1, pointer_state.y as i64 + dy));
                        pointer_state.delta_y += dy;
                        pointer_state.y = new_y;
                    }
                    8 => {
                        // Scroll wheel
                        let delta = (event.value as i32) as i64;
                        input_state.add_event(InputEvent::Scroll { delta });
                    }
                    _ => log::warn!("Unknown event code {} for pointer event", event.code),
                },

                _ => log::warn!("Unknown event type {}", event._type),
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
        FpsManager {
            fps_target,
            frame_start_t: 0.0,
            frametime: 1000.0 / fps_target,
            used: 0.0,
        }
    }

    fn start_frame(&mut self, clock: &SystemClock) {
        self.frame_start_t = clock.time();
    }

    fn end_frame<F: FbViewMut>(&mut self, 
        uitk_context: &mut uitk::UiContext<F>,
        clock: &SystemClock,
    ) {
        const SMOOTHING: f64 = 0.8;

        let UiContext { fb, font_family, .. } = uitk_context;
        let font = font_family.get_default();

        let frametime_target = 1000.0 / self.fps_target;

        let fps = 1000.0 / self.frametime;
        let s = format!("{:.2} FPS", fps);
        draw_str(*fb, &s, 0, 0, font, Color::WHITE, None);

        let char_h = font.char_h as u32;
        let graph_w = 12 * 9;
        let graph_h = 6;
        let used_frac = self.used / frametime_target;
        let used_w = (used_frac * graph_w as f64) as u32;
        let graph_color = {
            if 0.0 <= used_frac && used_frac < 0.50 {
                Color::GREEN
            } else if 0.50 <= used_frac && used_frac < 0.75 {
                Color::YELLOW
            } else {
                Color::RED
            }
        };
        draw_rect(
            *fb,
            &Rect {
                x0: 0,
                y0: char_h as i64,
                w: graph_w,
                h: 12,
            },
            Color::BLACK,
            false,
        );
        draw_rect(
            *fb,
            &Rect {
                x0: 0,
                y0: char_h as i64 + 3,
                w: used_w,
                h: graph_h,
            },
            graph_color,
            false,
        );

        let budget_color = if self.used < frametime_target {
            Color::WHITE
        } else {
            Color::RED
        };
        let budget_txt = format!("{:>6.2} ms", self.used);
        draw_str(
            *fb,
            &budget_txt,
            0,
            (char_h + graph_h + 6) as i64,
            font,
            budget_color,
            None,
        );

        let frame_end_t = clock.time();

        self.used = frame_end_t - self.frame_start_t;

        let new_frametime = match (self.used < frametime_target) && LIMIT_FPS {
            true => {
                clock.spin_delay(frametime_target - self.used);
                frametime_target
            }
            false => self.used,
        };

        self.frametime = SMOOTHING * self.frametime + (1.0 - SMOOTHING) * new_frametime;
    }
}

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    log::error!("{}", info);
    loop {}
}
