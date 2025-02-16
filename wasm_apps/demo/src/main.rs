extern crate alloc;

use alloc::collections::BTreeMap;
use applib::drawing::primitives::draw_rect;
use applib::drawing::text::{draw_str, Font, RichText, TextJustification, DEFAULT_FONT_FAMILY};
use applib::{Color, FbViewMut, Framebuffer, OwnedPixels};
use core::cell::OnceCell;
use guestlib::{PixelData, WasmLogger};
use applib::Rect;
use applib::content::TrackedContent;
use applib::uitk::{self, IconStore, ButtonConfig, ChoiceButtonsConfig, ChoiceConfig, EditableRichText, TextBoxState, UuidProvider};



const ICONS_PNG_DATA: [(&'static str, &[u8]); 3] = [
    ("justif_left", include_bytes!("../icons/justif_left.png")),
    ("justif_center", include_bytes!("../icons/justif_center.png")),
    ("justif_right", include_bytes!("../icons/justif_right.png")),
];



struct AppState {
    pixel_data: PixelData,
    ui_store: uitk::UiStore,
    uuid_provider: UuidProvider,
    icon_store: IconStore,

    textbox_text: TrackedContent<RichText>,
    textbox_prelude: TrackedContent<RichText>,
    textbox_state: TextBoxState,

    selected_justif: usize,
    selected_color: usize,
    selected_size: usize,
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

    let selected_justif = 0;
    let selected_color = 0;
    let selected_size = 0;

    let textbox_state = {
        let mut tb_state = TextBoxState::new();
        tb_state.justif = get_justif(selected_justif);
        tb_state
    };

    let color = get_color(selected_color);
    let font = get_font(selected_size);    

    let textbox_text = {
        let text = RichText::from_str("pouet\ntralala", color, font);
        TrackedContent::new(text, &mut uuid_provider)
    };

    let textbox_prelude = {
        let text = RichText::from_str("Write text here >>>", color, font);
        TrackedContent::new(text, &mut uuid_provider)
    };


    let state = AppState {
        pixel_data: PixelData::new(),
        ui_store: uitk::UiStore::new(),
        uuid_provider: UuidProvider::new(),

        icon_store: IconStore::new(&ICONS_PNG_DATA),


        textbox_text,
        textbox_prelude,
        textbox_state,
        selected_justif,
        selected_color,
        selected_size,
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

    draw_rect(
        uitk_context.fb,
        &Rect { x0: (w / 2).into(), y0: 0, w: w / 2, h },
        stylesheet.colors.background,
        false
    );

    let mut y = 0;
    let row_h = 40;

    // Justification

    uitk_context.choice_buttons_exclusive(
        &ChoiceButtonsConfig {
            rect: Rect { x0: (w / 2).into(), y0: y, w: 128, h: row_h },
            choices: vec![
                ChoiceConfig {
                    text: "".to_owned(),
                    icon: Some(state.icon_store.get("justif_left")),
                },
                ChoiceConfig {
                    text: "".to_owned(),
                    icon: Some(state.icon_store.get("justif_center")),
                },
                ChoiceConfig {
                    text: "".to_owned(),
                    icon: Some(state.icon_store.get("justif_right")),
                },
            ]
        },
        &mut state.selected_justif
    );

    state.textbox_state.justif = get_justif(state.selected_justif);

    y += row_h as i64;

    // Color

    uitk_context.choice_buttons_exclusive(
        &ChoiceButtonsConfig {
            rect: Rect { x0: (w / 2).into(), y0: y, w: 200, h: row_h },
            choices: vec![
                ChoiceConfig {
                    text: "White".to_owned(),
                    ..Default::default()
                },
                ChoiceConfig {
                    text: "Blue".to_owned(),
                    ..Default::default()
                },
                ChoiceConfig {
                    text: "Red".to_owned(),
                    ..Default::default()
                },
            ]
        },
        &mut state.selected_color
    );

    let color = get_color(state.selected_color);

    y += row_h as i64;

    // Font

    uitk_context.choice_buttons_exclusive(
        &ChoiceButtonsConfig {
            rect: Rect { x0: (w / 2).into(), y0: y, w: 60, h: row_h },
            choices: vec![
                ChoiceConfig {
                    text: "18".to_owned(),
                    ..Default::default()
                },
                ChoiceConfig {
                    text: "24".to_owned(),
                    ..Default::default()
                },
            ]
        },
        &mut state.selected_size
    );

    let font = get_font(state.selected_size);


    uitk_context.editable_text_box(
        &Rect { x0: 0, y0: 0, w: w / 2, h },
        &mut EditableRichText {
            color,
            font,
            rich_text: &mut state.textbox_text
        },
        &mut state.textbox_state,
        true,
        true,
        Some(&state.textbox_prelude)
    );

    // uitk_context.text_box(
    //     &Rect { x0: (w / 2) as i64, y0: 0, w: w / 2, h },
    //     &state.textbox_text,
    //     &mut state.textbox_2_state,
    //     true
    // );
}

fn get_justif(selected: usize) -> TextJustification {
    match selected {
        0 => TextJustification::Left,
        1 => TextJustification::Center,
        _ => TextJustification::Right,
    }
}

fn get_color(selected: usize) -> Color {
    match selected {
        0 => Color::WHITE,
        1 => Color::BLUE,
        _ => Color::RED,
    }
}

fn get_font(selected: usize) -> &'static Font {
    let size = match selected {
        0 => 18,
        _ => 24,
    };

    DEFAULT_FONT_FAMILY.by_size.get(&size).unwrap()
}

