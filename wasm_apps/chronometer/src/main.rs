extern crate alloc;

use alloc::format;
use applib::drawing::text::{draw_str, DEFAULT_FONT};
use applib::{Color, FbViewMut};
use core::cell::OnceCell;
use guestlib::PixelData;

mod drawing;
use drawing::draw_chrono;

struct AppState {
    pixel_data: PixelData,
}

static mut APP_STATE: OnceCell<AppState> = OnceCell::new();

fn main() {}

#[no_mangle]
pub fn init() -> () {
    let state = AppState { 
        pixel_data: PixelData::new()
    };
    unsafe {
        APP_STATE
            .set(state)
            .unwrap_or_else(|_| panic!("App already initialized"));
    }
}

#[no_mangle]
pub fn step() {
    let state = unsafe { APP_STATE.get_mut().expect("App not initialized") };

    let t = guestlib::get_time();

    let mut framebuffer = state.pixel_data.get_framebuffer();

    framebuffer.fill(Color::BLACK);
    draw_chrono(&mut framebuffer, t);

    let s = format!("{:.1}", t);
    draw_str(
        &mut framebuffer,
        &s,
        0,
        0,
        &DEFAULT_FONT,
        Color::YELLOW,
        None,
    );
}
