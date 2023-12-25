#![no_std]
#![no_main]

extern crate alloc;

use core::cell::OnceCell;
use alloc::{format, borrow::ToOwned};
use alloc::string::String;
use alloc::vec::Vec;
use guestlib::FramebufferHandle;
use applib::{Color, Rect};
use applib::input::InputEvent;
use applib::input::{Keycode, CHARMAP};
use applib::drawing::text::{draw_rich_text, RichText, HACK_15};
use applib::ui::button::{Button, ButtonConfig};

struct AppState {
    fb_handle: FramebufferHandle,
    input_buffer: String,
    console_buffer: Vec<EvalResult>,
    rhai_engine: rhai::Engine,
    button: Button,
    shift_pressed: bool,
}

#[derive(Debug)]
struct EvalResult {
    cmd: String,
    res: String,
    success: bool,
}

static mut APP_STATE: OnceCell<AppState> = OnceCell::new();

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
        input_buffer: String::with_capacity(100),
        console_buffer: Vec::with_capacity(500),
        rhai_engine: rhai::Engine::new(),
        button: Button::new(&ButtonConfig { 
            text: "Clear".to_owned(),
            ..Default::default()
        }),
        shift_pressed: false,
    };
    unsafe { APP_STATE.set(state).unwrap_or_else(|_| panic!("App already initialized")) }
}

#[no_mangle]
pub fn step() {

    let state = unsafe { APP_STATE.get_mut().expect("App not initialized") };

    let system_state = guestlib::get_system_state();
    let mut framebuffer = guestlib::get_framebuffer(&mut state.fb_handle);

    let win_rect = guestlib::get_win_rect();
    let Rect { w: win_w, h: win_h, .. } = win_rect;

    let input_state = &system_state.input;


    //
    // Updating shift state

    let check_is_shift = |keycode| {
        keycode == Keycode::KEY_LEFTSHIFT || 
        keycode == Keycode::KEY_RIGHTSHIFT
    };
    input_state.events.iter().for_each(|&event| match event {
        Some(InputEvent::KeyPress { keycode }) if check_is_shift(keycode) => state.shift_pressed = true,
        Some(InputEvent::KeyRelease { keycode }) if check_is_shift(keycode) => state.shift_pressed = false,
        _ => ()
    });


    //
    // Reading keypress events

    for event in input_state.events {

        match event {

            // Enter key pressed (flushing input)
            Some(InputEvent::KeyPress { keycode: Keycode::KEY_ENTER }) => {
                if !state.input_buffer.is_empty() {

                    let cmd = state.input_buffer.to_owned();

                    let result = match state.rhai_engine.eval::<rhai::Dynamic>(&cmd) {
                        Ok(res) => EvalResult { cmd, res: format!("{:?}", res), success: true },
                        Err(res) => EvalResult { cmd, res: format!("{:?}", res), success: false },
                    };
                    
                    //state.console_buffer.push_str(&format!("$ {}\n  > {}\n", state.input_buffer, result));
                    state.console_buffer.push(result);
                    state.input_buffer.clear();
                }
            },

            // Character input
            Some(InputEvent::KeyPress { keycode }) => {

                let new_char = CHARMAP
                    .get(&keycode)
                    .map(|(low_c, up_c)| if state.shift_pressed { *up_c } else { *low_c })
                    .flatten();

                if let Some(new_char) = new_char {
                    state.input_buffer.push(new_char);
                }
            }

            _ => ()
        };
    }

    let win_input_state = system_state.input.change_origin(&win_rect);
    let clear_console = state.button.update(&win_input_state);
    if clear_console {
        state.console_buffer.clear();
    }

    framebuffer.fill(Color::from_rgba(0, 0, 0, 0xff));

    let font = &HACK_15;

    let char_h = font.char_h as u32;
    let rect_console = Rect  { x0: 0, y0: 0, w: win_w, h: win_h - char_h};
    let rect_input = Rect  { x0: 0, y0: (win_h - char_h) as i64, w: win_w, h: char_h};

    let mut console_rich_text = RichText::new();
    for res in state.console_buffer.iter() {
        console_rich_text.add_part("$ ", YELLOW, font);
        console_rich_text.add_part(&res.cmd, WHITE, font);
        let res_color = if res.success { WHITE } else { RED };
        console_rich_text.add_part(&format!("\n  > {}", res.res), res_color, font);
        console_rich_text.add_part("\n", WHITE, font)
    }
    
    draw_rich_text(&mut framebuffer, &console_rich_text, &rect_console, 0);

    let mut input_rich_text = RichText::new();
    input_rich_text.add_part("> ", YELLOW, font);
    input_rich_text.add_part(&state.input_buffer, WHITE, font);
    draw_rich_text(&mut framebuffer, &input_rich_text, &rect_input, 0);

    state.button.draw(&mut framebuffer);
}
