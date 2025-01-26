extern crate alloc;

use alloc::format;
use applib::drawing::text::{draw_str, DEFAULT_FONT_FAMILY};
use applib::{Color, FbViewMut};
use core::cell::OnceCell;
use guestlib::{PixelData, WasmLogger};
use applib::Rect;
use applib::content::TrackedContent;
use applib::uitk::{self, UuidProvider, TextBoxState};

struct AppState {
    pixel_data: PixelData,
    ui_store: uitk::UiStore,
    uuid_provider: UuidProvider,
    textbox_text: TrackedContent<String>,
    textbox_prelude: TrackedContent<String>,
    textbox_state: TextBoxState,
}

static mut APP_STATE: OnceCell<AppState> = OnceCell::new();

static LOGGER: WasmLogger = WasmLogger;
const LOGGING_LEVEL: log::LevelFilter = log::LevelFilter::Debug;

fn main() {}

#[no_mangle]
pub fn init() -> () {

    log::set_max_level(LOGGING_LEVEL);
    log::set_logger(&LOGGER).unwrap();

    let mut uuid_provider = uitk::UuidProvider::new();
    let textbox_text = TrackedContent::new("pouet\ntralala".to_owned(), &mut uuid_provider);
    let textbox_prelude = TrackedContent::new(
        "Write text here >>>".to_string(),
        &mut uuid_provider,
    );

    let state = AppState {
        pixel_data: PixelData::new(),
        ui_store: uitk::UiStore::new(),
        uuid_provider: UuidProvider::new(),
        textbox_text,
        textbox_prelude,
        textbox_state: TextBoxState::new(),
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

    let time = guestlib::get_time();
    let stylesheet = guestlib::get_stylesheet();
    let input_state = guestlib::get_input_state();
    let Rect { w, h, ..} = guestlib::get_win_rect();


    let mut framebuffer = state.pixel_data.get_framebuffer();

    let mut uitk_context = state.ui_store.get_context(
        &mut framebuffer,
        &stylesheet,
        &input_state,
        &mut state.uuid_provider,
        time
    );

    uitk_context.text_box(
        &Rect { x0: 0, y0: 0, w: w / 2, h },
        &mut state.textbox_text,
        &mut state.textbox_state,
        true,
        true,
        true,
        Some(&state.textbox_prelude)
    );
}
