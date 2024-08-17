#![feature(vec_into_raw_parts)]

extern crate alloc;

use core::cell::OnceCell;
use alloc::{format, borrow::ToOwned};
use alloc::string::String;
use alloc::vec::Vec;
use guestlib::FramebufferHandle;
use guestlib::{WasmLogger};
use applib::{Color, Framebuffer, Rect};
use applib::input::InputEvent;
use applib::input::{Keycode, CHARMAP, InputState};
use applib::drawing::text::{RichText, HACK_15, Font};
use applib::uitk::{self, TrackedContent};

mod python;

#[derive(Debug)]
struct EvalResult {
    cmd: String,
    pyres: python::EvalResult,
}

static LOGGER: WasmLogger = WasmLogger;
const LOGGING_LEVEL: log::LevelFilter = log::LevelFilter::Debug;

struct AppState<'a> {
    fb_handle: FramebufferHandle,
    input_buffer: TrackedContent<String>,
    console_buffer: TrackedContent<Vec<EvalResult>>,

    text_fb: Framebuffer<'a>,
    scroll_offsets: (i64, i64),
    dragging: bool,

    python: python::Python,
    caps: bool,
    content_ids: Option<[uitk::ContentId; 2]>,
}

static mut APP_STATE: OnceCell<AppState> = OnceCell::new();

fn main() {}

#[no_mangle]
pub fn init() -> () {

    log::set_max_level(LOGGING_LEVEL);
    log::set_logger(&LOGGER).unwrap();

    let win_rect = guestlib::get_win_rect();
    let fb_handle = guestlib::create_framebuffer(win_rect.w, win_rect.h);

    let Rect { w: win_w, h: win_h, .. } = win_rect;

    let state = AppState { 
        fb_handle,
        input_buffer: TrackedContent::new(String::new()),
        console_buffer: TrackedContent::new(Vec::with_capacity(500)),
        text_fb: Framebuffer::new_owned(win_w, 4 * win_h),
        scroll_offsets: (0, 0),
        dragging: false,
        python: python::Python::new(),
        caps: false,
        content_ids: None,
    };
    unsafe { APP_STATE.set(state).unwrap_or_else(|_| panic!("App already initialized")) }
}

#[no_mangle]
pub fn step() {

    let state = unsafe { APP_STATE.get_mut().expect("App not initialized") };

    let system_state = guestlib::get_system_state();
    let mut framebuffer = state.fb_handle.as_framebuffer();

    let win_rect = guestlib::get_win_rect();
    let input_state = system_state.input.change_origin(&win_rect);

    uitk::string_input(&mut state.input_buffer, &mut state.caps, &input_state, false);

    let mut autoscroll = false;
    if check_enter_pressed(&input_state) && !state.input_buffer.as_ref().is_empty() {
        let cmd = state.input_buffer.as_ref().to_owned();
        let pyres = state.python.run_code(&cmd);
        state.console_buffer.mutate().push(EvalResult { cmd, pyres });
        state.input_buffer.mutate().clear();
        autoscroll = true;
    }

    let Rect { w: win_w, h: win_h, .. } = win_rect;
    let rect_console = Rect  { x0: 0, y0: 0, w: win_w, h: win_h };

    let redraw = state.content_ids != Some([state.input_buffer.get_id(), state.console_buffer.get_id()]);

    if redraw {
        let console_rich_text = render_console(state.input_buffer.as_ref(), state.console_buffer.as_ref());
        uitk::render_rich_text(&mut state.text_fb, &console_rich_text);
    }

    if autoscroll {
        uitk::set_autoscroll(&rect_console, &state.text_fb, &mut state.scroll_offsets);
    }

    uitk::scrollable_canvas(
        &mut framebuffer,
        &rect_console,
        &state.text_fb,
        &mut state.scroll_offsets,
        &input_state,
        &mut state.dragging,
    );

    state.content_ids = Some([
        state.input_buffer.get_id(),
        state.console_buffer.get_id(),
    ]);
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

fn render_console(input_buffer: &String, console_buffer: &Vec<EvalResult>) -> RichText {

    let font = &HACK_15;

    let mut console_rich_text = RichText::new();

    for res in console_buffer.iter() {

        console_rich_text.add_part(">>> ", Color::YELLOW, font, None);
        console_rich_text.add_part(&res.cmd, Color::WHITE, font, None);

        let color = match &res.pyres {
            python::EvalResult::Failure(_) => Color::RED,
            _ => Color::WHITE
        };

        let text = match &res.pyres {
            python::EvalResult::Failure(err) => format!("\n{}", err),
            python::EvalResult::Success(repr) => format!("\n{}", repr),
        };

        console_rich_text.add_part(&text, color, font, None);
        console_rich_text.add_part("\n", Color::WHITE, font, None)
    }

    console_rich_text.add_part(">>> ", Color::WHITE, font, None);
    console_rich_text.add_part(input_buffer.as_ref(), Color::WHITE, font, None);

    console_rich_text
}
