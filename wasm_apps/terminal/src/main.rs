extern crate alloc;

use core::cell::OnceCell;
use alloc::{format, borrow::ToOwned};
use alloc::string::String;
use alloc::vec::Vec;
use guestlib::FramebufferHandle;
use applib::{Color, Rect};
use applib::input::InputEvent;
use applib::input::{Keycode, CHARMAP, InputState};
use applib::drawing::text::{RichText, HACK_15, Font};
use applib::ui::text::{ScrollableText, TextConfig, string_input};

mod python;

struct EvalResult {
    cmd: String,
    pyres: python::EvalResult,
}

struct AppState {
    fb_handle: FramebufferHandle,
    input_buffer: String,
    console_buffer: Vec<EvalResult>,
    python: python::Python,

    font: &'static Font,

    console_area: ScrollableText,

    caps: bool,
}

static mut APP_STATE: OnceCell<AppState> = OnceCell::new();

fn main() {}

#[no_mangle]
pub fn init() -> () {

    let win_rect = guestlib::get_win_rect();
    let fb_handle = guestlib::create_framebuffer(win_rect.w, win_rect.h);

    let Rect { w: win_w, h: win_h, .. } = win_rect;
    let font = &HACK_15;

    let rect_console = Rect  { x0: 0, y0: 0, w: win_w, h: win_h };

    let state = AppState { 
        fb_handle,
        input_buffer: String::with_capacity(100),
        console_buffer: Vec::with_capacity(500),
        python: python::Python::new(),
        font,
        console_area: ScrollableText::new(&TextConfig { 
            rect: rect_console,
            ..Default::default()
        }),
        caps: false,
    };
    unsafe { APP_STATE.set(state).unwrap_or_else(|_| panic!("App already initialized")) }
}

#[no_mangle]
pub fn step() {

    let state = unsafe { APP_STATE.get_mut().expect("App not initialized") };

    let system_state = guestlib::get_system_state();
    let mut framebuffer = guestlib::get_framebuffer(&mut state.fb_handle);

    let win_rect = guestlib::get_win_rect();

    let input_state = &system_state.input;

    string_input(&mut state.input_buffer, &mut state.caps, &input_state, false);

    if check_enter_pressed(input_state) && !state.input_buffer.is_empty() {
        let cmd = state.input_buffer.to_owned();

        let pyres = state.python.run_code(&cmd);
        
        state.console_buffer.push(EvalResult { cmd, pyres });
        state.input_buffer.clear();
    }

    let font = state.font;

    let win_input_state = system_state.input.change_origin(&win_rect);

    let console_rich_text = {

        let mut console_rich_text = RichText::new();

        for res in state.console_buffer.iter() {

            console_rich_text.add_part(">>> ", Color::YELLOW, font, None);
            console_rich_text.add_part(&res.cmd, Color::WHITE, font, None);

            let color = match &res.pyres {
                python::EvalResult::Failure(_) => Color::RED,
                _ => Color::WHITE
            };

            let text = match &res.pyres {
                python::EvalResult::Failure(err) => format!("\n{}", err),
                python::EvalResult::Success(repr) => format!("\n{}", repr),
                python::EvalResult::None => "".to_owned()
            };

            console_rich_text.add_part(&text, color, font, None);
            console_rich_text.add_part("\n", Color::WHITE, font, None)
        }

        console_rich_text.add_part(">>> ", Color::WHITE, font, None);
        console_rich_text.add_part(&state.input_buffer, Color::WHITE, font, None);

        Some(console_rich_text)
    };

    state.console_area.update(&win_input_state, console_rich_text);

    framebuffer.fill(Color::BLACK);

    state.console_area.draw(&mut framebuffer);
}

fn check_enter_pressed(input_state: &InputState) -> bool {
    input_state.events.iter().any(|event| {
        if let Some(InputEvent::KeyPress { keycode: Keycode::KEY_ENTER }) = event {
            true
        } else {
            false
        }
    })
}
