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

fn main() {}

#[no_mangle]
pub fn init() -> () {

    // TESTING
    // println!("Connecting TCP");
    // guestlib::tcp_connect();

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

    framebuffer.fill(Color::BLACK);
    draw_chrono(&mut framebuffer, system_state.time);

    let s = format!("{:.1}", system_state.time);
    draw_str(&mut framebuffer, &s, 0, 0, &DEFAULT_FONT, Color::YELLOW);
}
