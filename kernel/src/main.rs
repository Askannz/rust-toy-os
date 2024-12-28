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

pub const FPS_TARGET: f64 = 60.0;
const LIMIT_FPS: bool = true;

static LOGGER: logging::SerialLogger = logging::SerialLogger;
const LOGGING_LEVEL: log::LevelFilter = log::LevelFilter::Debug;

pub const TOPBAR_H: u32 = 40;

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
    
    let alloc_stats = memory::ALLOCATOR.get_stats();

    let system_stats = stats::SystemStats::new(&alloc_stats, &app_names);

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


        let mut uitk_context = ui_store.get_context(
            &mut framebuffer,
            &system.stylesheet,
            &input_state,
            &mut uuid_provider,
            time
        );

        run_apps(
            &mut uitk_context,
            &mut system,
            &wasm_engine,
            &mut apps_manager,
            &input_state,
            &mut apps_interaction_state,
        );

        topbar::topbar(&mut uitk_context, &system.stats, datetime);

        draw_cursor(uitk_context.fb, &input_state);

        let (net_recv, net_sent) = system.tcp_stack.pop_counters();

        let t1 = system.clock.time();

        let heap_stats = memory::ALLOCATOR.get_stats();

        *system.stats.get_system_point_mut() = stats::SystemDataPoint {
            alloc: heap_stats,
            frametime_used: t1 - t0,
            net_recv,
            net_sent,
        };

        system.stats.next_frame();
        fps_manager.end_frame(&system.clock);
        virtio_gpu.flush();
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

    fn end_frame(&mut self, clock: &SystemClock) {

        const SMOOTHING: f64 = 0.8;

        let frametime_target = 1000.0 / self.fps_target;

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
