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
use applib::uitk::{self, UiStore, UuidProvider, TextBoxState, EditableRichText};
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

    input_buffer: TrackedContent<RichText>,
    history: TrackedContent<Vec<EvalResult>>,

    ui_store: UiStore,
    uuid_provider: uitk::UuidProvider,
    textbox_state: TextBoxState,

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
        input_buffer: TrackedContent::new(RichText::new(), &mut uuid_provider),
        history: TrackedContent::new(Vec::new(), &mut uuid_provider),
        ui_store: uitk::UiStore::new(),
        uuid_provider,
        textbox_state: TextBoxState::new(),
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

    let rich_text_prelude = get_rich_text_prelude(&stylesheet, &DEFAULT_FONT_FAMILY, &state.history);

    let time = guestlib::get_time();

    let mut uitk_context = state.ui_store.get_context(
        &mut framebuffer,
        &stylesheet,
        &input_state,
        &mut state.uuid_provider,
        time
    );

    let Rect {
        w: win_w, h: win_h, ..
    } = win_rect;

    let rect_console = Rect {
        x0: 0,
        y0: 0,
        w: win_w,
        h: win_h,
    };

    let font = DEFAULT_FONT_FAMILY.get_default();

    let mut editable = EditableRichText { 
        rich_text: &mut state.input_buffer,
        font,
        color: Color::WHITE
    };

    uitk_context.text_box(
        &rect_console,
        &mut editable,
        &mut state.textbox_state,
        true,
        true,
        false,
        Some(&rich_text_prelude),
    );

    if input_state.check_key_pressed(Keycode::KEY_ENTER) && !state.input_buffer.as_ref().is_empty() {
        let cmd = state.input_buffer.as_ref().as_string();
        let pyres = state.python.run_code(&cmd);
        state
            .history
            .mutate(&mut state.uuid_provider)
            .push(EvalResult { cmd, pyres });
        state.input_buffer.mutate(&mut state.uuid_provider).clear();
    }
}

fn get_rich_text_prelude(
    stylesheet: &StyleSheet,
    font_family: &'static FontFamily,
    history: &TrackedContent<Vec<EvalResult>>,
) -> TrackedContent<RichText> {

    let font = font_family.get_default();

    let mut rich_text = RichText::new();

    for res in history.as_ref().iter() {
        rich_text.add_part(">>> ", stylesheet.colors.yellow, font);
        rich_text.add_part(&res.cmd, stylesheet.colors.text, font);

        let color = match &res.pyres {
            python::EvalResult::Failure(_) => Color::rgb(200, 150, 25),
            _ => stylesheet.colors.text,
        };

        let text = match &res.pyres {
            python::EvalResult::Failure(err) => format!("\n{}", err),
            python::EvalResult::Success(repr) => format!("\n{}", repr),
        };

        rich_text.add_part(&text, color, font);
        rich_text.add_part("\n", stylesheet.colors.text, font)
    }

    rich_text.add_part(">>> ", stylesheet.colors.text, font);

    TrackedContent::new_with_id(rich_text, history.get_id())
}
