#![no_std]
#![no_main]

extern crate alloc;

use core::cell::OnceCell;
use alloc::{format, borrow::ToOwned};
use alloc::string::String;
use alloc::vec::Vec;
use guestlib::FramebufferHandle;
use applib::{Color, Rect};
use applib::keymap::{Keycode, CHARMAP};
use applib::drawing::text::{draw_rich_text, RichText, HACK_15};

#[derive(Debug)]
struct AppState {
    fb_handle: FramebufferHandle,
    input_buffer: String,
    console_buffer: Vec<EvalResult>,
    last_input_t: f64,
    rhai_engine: rhai::Engine,
}

#[derive(Debug)]
enum EvalResult {
    Success { cmd: String, res: String},
    Error { cmd: String }
}

static mut APP_STATE: OnceCell<AppState> = OnceCell::new();

const INPUT_RATE_PERIOD: f64 = 100.0;

const WHITE: Color = Color::from_rgba(255, 255, 255, 255);
const RED: Color = Color::from_rgba(255, 0, 0, 255);
const GREEN: Color = Color::from_rgba(0, 255, 0, 255);
const YELLOW: Color = Color::from_rgba(255, 255, 0, 255);

#[no_mangle]
pub fn init() -> () {
    let win_rect = guestlib::get_win_rect();
    let fb_handle = guestlib::create_framebuffer(win_rect.w, win_rect.h);
    let state = AppState { 
        fb_handle,
        input_buffer: String::with_capacity(20),
        console_buffer: Vec::new(),
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

        let cmd = state.input_buffer.to_owned();

        let result = match state.rhai_engine.eval::<rhai::Dynamic>(&cmd).ok() {
            Some(res) => EvalResult::Success { cmd, res: format!("{:?}", res) },
            None => EvalResult::Error { cmd },
        };
        
        //state.console_buffer.push_str(&format!("$ {}\n  > {}\n", state.input_buffer, result));
        state.console_buffer.push(result);
        state.input_buffer.clear();
    }

    let mut framebuffer = guestlib::get_framebuffer(&mut state.fb_handle);

    framebuffer.fill(Color::from_rgba(0, 0, 0, 0xff));

    let font = &HACK_15;

    let char_h = font.char_h as u32;
    let rect_console = Rect  { x0: 0, y0: 0, w: win_w, h: win_h - char_h};
    let rect_input = Rect  { x0: 0, y0: (win_h - char_h) as i64, w: win_w, h: char_h};

    let mut console_rich_text = RichText::new();
    for res in state.console_buffer.iter() {
        match res {
            EvalResult::Success { cmd, res } => {
                console_rich_text.add_part("$ ", GREEN, font);
                console_rich_text.add_part(&cmd, WHITE, font);
                console_rich_text.add_part(&format!("\n  > {}", res), WHITE, font);
            },
            EvalResult::Error { cmd } => {
                console_rich_text.add_part("$ ", RED, font);
                console_rich_text.add_part(&cmd, WHITE, font);
                console_rich_text.add_part(&format!("\n  > ERROR"), RED, font);
            }
        }
        console_rich_text.add_part("\n", WHITE, font)
    }
    
    draw_rich_text(&mut framebuffer, &console_rich_text, &rect_console);

    let mut input_rich_text = RichText::new();
    input_rich_text.add_part("> ", YELLOW, font);
    input_rich_text.add_part(&state.input_buffer, WHITE, font);
    draw_rich_text(&mut framebuffer, &input_rich_text, &rect_input);
}
