#![no_std]
#![no_main]

extern crate alloc;

use core::cell::OnceCell;
use alloc::format;
use alloc::vec::Vec;
use guestlib::{FramebufferHandle, println};
use applib::{draw_str, DEFAULT_FONT, Color, keymap::Keycode};

mod drawing;
use drawing::draw_chrono;

#[derive(Debug)]
struct AppState {
    fb_handle: FramebufferHandle
}

static mut APP_STATE: OnceCell<AppState> = OnceCell::new();

const W: usize = 200;
const H: usize = 200;

#[no_mangle]
pub fn init() -> () {
    let fb_handle = guestlib::create_framebuffer(W, H);
    let state = AppState { fb_handle };
    unsafe { APP_STATE.set(state).expect("App already initialized"); }
}

#[no_mangle]
pub fn step() {

    let state = unsafe { APP_STATE.get_mut().expect("App not initialized") };

    let system_state = guestlib::get_system_state();

    //println!("{:?}", system_state.keyboard);

    let mut framebuffer = guestlib::get_framebuffer(&mut state.fb_handle);

    framebuffer.fill(&[0u8; 4]);
    draw_chrono(&mut framebuffer, system_state.time);

    let s = format!("{}", system_state.time);
    draw_str(&mut framebuffer, &s, 0, 0, &DEFAULT_FONT, &Color(255, 255, 0));
}
