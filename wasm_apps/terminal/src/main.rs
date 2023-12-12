#![no_std]
#![no_main]

extern crate alloc;

use core::cell::OnceCell;
use alloc::{format, borrow::ToOwned};
use alloc::string::String;
use guestlib::FramebufferHandle;
use applib::{Color, Rect};
use applib::keymap::{Keycode, CHARMAP};
use applib::drawing::text::{draw_text_rect, HACK_15};

#[derive(Debug)]
struct AppState {
    fb_handle: FramebufferHandle,
    input_buffer: String,
    console_buffer: String,
    last_input_t: f64,
    rhai_engine: rhai::Engine,
}

static mut APP_STATE: OnceCell<AppState> = OnceCell::new();

const INPUT_RATE_PERIOD: f64 = 100.0;

#[no_mangle]
pub fn init() -> () {
    let win_rect = guestlib::get_win_rect();
    let fb_handle = guestlib::create_framebuffer(win_rect.w, win_rect.h);
    let state = AppState { 
        fb_handle,
        input_buffer: String::with_capacity(20),
        console_buffer: String::with_capacity(800),
        last_input_t: 0.0,
        rhai_engine: rhai::Engine::new(),
    };
    unsafe { APP_STATE.set(state).expect("App already initialized"); }
}

#[no_mangle]
pub fn step() {

    let state = unsafe { APP_STATE.get_mut().expect("App not initialized") };

    let system_state = guestlib::get_system_state();
    let Rect { w: win_w, h: win_h, .. } = guestlib::get_win_rect();

    //println!("{:?} {}", system_state.keyboard, state.s);

    let shift_pressed = system_state.keyboard.contains(&Some(Keycode::KEY_LEFTSHIFT));
    let enter_pressed = system_state.keyboard.contains(&Some(Keycode::KEY_ENTER));

    let new_char = system_state.keyboard
        .iter()
        .find_map(|keycode| match keycode {
            &Some(keycode) => CHARMAP
                    .get(&keycode)
                    .map(|(low_c, up_c)| if shift_pressed { *up_c } else { *low_c })
                    .flatten(),
            None => None
        });

    if let Some(new_char) = new_char {
        let curr_input_t = system_state.time;
        if curr_input_t - state.last_input_t > INPUT_RATE_PERIOD {
            state.input_buffer.push(new_char);
            state.last_input_t = curr_input_t;
        } 
    } 

    if enter_pressed && !state.input_buffer.is_empty() {

        let result: String = match state.rhai_engine.eval::<rhai::Dynamic>(&state.input_buffer).ok() {
            Some(res) => format!("{:?}", res),
            None => "ERROR".to_owned(),
        };
        
        state.console_buffer.push_str(&format!("$ {}\n  > {}\n", state.input_buffer, result));
        state.input_buffer.clear();
    }

    let mut framebuffer = guestlib::get_framebuffer(&mut state.fb_handle);

    framebuffer.fill(Color::from_rgba(0, 0, 0, 0xff));

    let font = &HACK_15;

    let char_h = font.char_h as u32;
    let rect_console = Rect  { x0: 0, y0: 0, w: win_w, h: win_h - char_h};
    let rect_input = Rect  { x0: 0, y0: (win_h - char_h) as i64, w: win_w, h: char_h};

    draw_text_rect(&mut framebuffer, &state.console_buffer, &rect_console, font, Color::from_rgba(255, 255, 255, 255));

    let input_fmt = format!("> {}", state.input_buffer);
    draw_text_rect(&mut framebuffer, &input_fmt, &rect_input, font, Color::from_rgba(255, 255, 255, 255));
}
