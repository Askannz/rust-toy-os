#![feature(vec_into_raw_parts)]

extern crate alloc;

use alloc::string::String;
use alloc::vec::Vec;
use alloc::{borrow::ToOwned, format};
use applib::content::{ContentId, TrackedContent};
use applib::drawing::text::{
    draw_rich_slice, format_rich_lines, FontFamily, FormattedRichText, RichText, DEFAULT_FONT_FAMILY
};
use applib::input::InputEvent;
use applib::input::{InputState, Keycode};
use applib::uitk::{self, UiStore, UuidProvider, ScrollableTextState};
use applib::{Color, FbViewMut, Rect, StyleSheet};
use core::cell::OnceCell;
use guestlib::PixelData;
use guestlib::WasmLogger;

mod python;

#[derive(Debug)]
struct EvalResult {
    cmd: String,
    pyres: python::EvalResult,
}

static LOGGER: WasmLogger = WasmLogger;
const LOGGING_LEVEL: log::LevelFilter = log::LevelFilter::Debug;

struct AppState {
    pixel_data: PixelData,

    input_buffer: TrackedContent<String>,
    console_buffer: TrackedContent<Vec<EvalResult>>,

    ui_store: UiStore,
    uuid_provider: uitk::UuidProvider,
    scrollable_text_state: ScrollableTextState,

    python: python::Python,
}

static mut APP_STATE: OnceCell<AppState> = OnceCell::new();

fn main() {}

#[no_mangle]
pub fn init() -> () {
    log::set_max_level(LOGGING_LEVEL);
    log::set_logger(&LOGGER).unwrap();

    let mut uuid_provider = uitk::UuidProvider::new();

    let state = AppState {
        pixel_data: PixelData::new(),
        input_buffer: TrackedContent::new(String::new(), &mut uuid_provider),
        console_buffer: TrackedContent::new(Vec::new(), &mut uuid_provider),
        ui_store: uitk::UiStore::new(),
        uuid_provider: UuidProvider::new(),
        scrollable_text_state: ScrollableTextState::new(),
        python: python::Python::new(),
    };
    unsafe {
        APP_STATE
            .set(state)
            .unwrap_or_else(|_| panic!("App already initialized"))
    }
}

#[no_mangle]
pub fn step() {
    let state = unsafe { APP_STATE.get_mut().expect("App not initialized") };

    let input_state = guestlib::get_input_state();
    let mut framebuffer = state.pixel_data.get_framebuffer();

    let win_rect = guestlib::get_win_rect();
    let stylesheet = guestlib::get_stylesheet();

    let mut cursor = state.input_buffer.as_ref().len();
    uitk::string_input(
        &mut state.input_buffer,
        &input_state,
        false,
        &mut cursor,
        &mut state.uuid_provider,
    );

    if check_enter_pressed(&input_state) && !state.input_buffer.as_ref().is_empty() {
        let cmd = state.input_buffer.as_ref().to_owned();
        let pyres = state.python.run_code(&cmd);
        state
            .console_buffer
            .mutate(&mut state.uuid_provider)
            .push(EvalResult { cmd, pyres });
        state.input_buffer.mutate(&mut state.uuid_provider).clear();
    }

    let Rect {
        w: win_w, h: win_h, ..
    } = win_rect;
    let rect_console = Rect {
        x0: 0,
        y0: 0,
        w: win_w,
        h: win_h,
    };

    let rich_text = get_rich_text(&stylesheet, &DEFAULT_FONT_FAMILY, &state.input_buffer, &state.console_buffer);

    let time = guestlib::get_time();

    let mut uitk_context = state.ui_store.get_context(
        &mut framebuffer,
        &stylesheet,
        &input_state,
        &mut state.uuid_provider,
        time
    );

    uitk_context.scrollable_text(
        &rect_console,
        &rich_text,
        &mut state.scrollable_text_state,
        true,
    );
}

fn check_enter_pressed(input_state: &InputState) -> bool {
    input_state.events.iter().any(|event| {
        if let Some(InputEvent::KeyPress {
            keycode: Keycode::KEY_ENTER,
        }) = event
        {
            true
        } else {
            false
        }
    })
}


fn get_rich_text(
    stylesheet: &StyleSheet,
    font_family: &'static FontFamily,
    input_buffer: &TrackedContent<String>,
    console_buffer: &TrackedContent<Vec<EvalResult>>,
) -> TrackedContent<RichText> {

    let font = font_family.get_default();

    let mut console_rich_text = RichText::new();

    for res in console_buffer.as_ref().iter() {
        console_rich_text.add_part(">>> ", stylesheet.colors.yellow, font);
        console_rich_text.add_part(&res.cmd, stylesheet.colors.text, font);

        let color = match &res.pyres {
            python::EvalResult::Failure(_) => Color::rgb(200, 150, 25),
            _ => stylesheet.colors.text,
        };

        let text = match &res.pyres {
            python::EvalResult::Failure(err) => format!("\n{}", err),
            python::EvalResult::Success(repr) => format!("\n{}", repr),
        };

        console_rich_text.add_part(&text, color, font);
        console_rich_text.add_part("\n", stylesheet.colors.text, font)
    }

    console_rich_text.add_part(">>> ", stylesheet.colors.text, font);
    console_rich_text.add_part(input_buffer.as_ref(), stylesheet.colors.text, font);

    let new_cid = ContentId::from_hash((
        input_buffer.get_id(),
        console_buffer.get_id(),
    ));

    TrackedContent::new_with_id(console_rich_text, new_cid)
}
