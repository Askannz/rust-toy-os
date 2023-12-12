#![no_std]
#![no_main]

extern crate alloc;

use core::cell::OnceCell;
use alloc::format;
use guestlib::FramebufferHandle;
use applib::Color;
use applib::drawing::text::{draw_str, DEFAULT_FONT};

mod drawing;
use drawing::draw_chrono;

#[derive(Debug)]
struct AppState {
    fb_handle: FramebufferHandle,
}

static mut APP_STATE: OnceCell<AppState> = OnceCell::new();

#[no_mangle]
pub fn init() -> () {
    let win_rect = guestlib::get_win_rect();
    let fb_handle = guestlib::create_framebuffer(win_rect.w, win_rect.h);
    let state = AppState { fb_handle };
    unsafe { APP_STATE.set(state).expect("App already initialized"); }
}

#[no_mangle]
pub fn step() {

    let state = unsafe { APP_STATE.get_mut().expect("App not initialized") };

    let system_state = guestlib::get_system_state();

    let mut framebuffer = guestlib::get_framebuffer(&mut state.fb_handle);

    framebuffer.fill(Color::from_rgba(0, 0, 0, 0xFF));
    draw_chrono(&mut framebuffer, system_state.time);

    let s = format!("{:.1}", system_state.time);
    draw_str(&mut framebuffer, &s, 0, 0, &DEFAULT_FONT, Color::from_rgba(255, 255, 0, 0xFF));
}
