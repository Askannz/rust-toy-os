#![no_std]
#![no_main]

extern crate alloc;

use core::cell::OnceCell;
use alloc::format;
use alloc::string::String;
use guestlib::{FramebufferHandle, println};
use applib::{draw_str, DEFAULT_FONT, Color, keymap::{Keycode, CHARMAP}};

#[derive(Debug)]
struct AppState {
    fb_handle: FramebufferHandle,
    s: String,
    last_input_t: f64,
}

static mut APP_STATE: OnceCell<AppState> = OnceCell::new();

const W: usize = 200;
const H: usize = 200;

const INPUT_RATE_PERIOD: f64 = 100.0;

#[no_mangle]
pub fn init() -> () {
    let fb_handle = guestlib::create_framebuffer(W, H);
    let state = AppState { fb_handle, s: String::with_capacity(800), last_input_t: 0.0};
    unsafe { APP_STATE.set(state).expect("App already initialized"); }
}

#[no_mangle]
pub fn step() {

    let state = unsafe { APP_STATE.get_mut().expect("App not initialized") };

    let system_state = guestlib::get_system_state();

    let curr_input_t = system_state.time;
    if curr_input_t - state.last_input_t > INPUT_RATE_PERIOD {

        let mut updated = false;
        let shift = system_state.keyboard.contains(&Some(Keycode::KEY_LEFTSHIFT));
        for keycode in system_state.keyboard.iter() {
            if let Some(keycode) = keycode {
    
                let c = CHARMAP
                    .get(keycode)
                    .map(|(low_c, up_c)| if shift { *up_c } else { *low_c })
                    .flatten();
    
                if let Some(c) = c {
                    updated = true;
                    state.s.push(c);
                }
            }
        }

        if updated { state.last_input_t = curr_input_t; }
    }

    //println!("{} {}", state.s, state.s.len());

    let mut framebuffer = guestlib::get_framebuffer(&mut state.fb_handle);

    framebuffer.fill(&[0u8; 4]);

    draw_str(&mut framebuffer, &state.s, 0, DEFAULT_FONT.char_h as u32, &DEFAULT_FONT, &Color(255, 255, 255));
}
