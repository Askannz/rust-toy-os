#![feature(vec_into_raw_parts)]

extern crate alloc;

use alloc::string::String;
use alloc::vec::Vec;
use alloc::{borrow::ToOwned, format};
use applib::content::{ContentId, TrackedContent};
use applib::drawing::text::{
    draw_rich_slice, format_rich_lines, FormattedRichText, RichText, HACK_15,
};
use applib::input::InputEvent;
use applib::input::{InputState, Keycode};
use applib::uitk::{self, UiStore, UuidProvider};
use applib::{Color, FbViewMut, Rect};
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

    text_formatter: ConsoleTextFormatter,

    ui_store: UiStore,
    uuid_provider: uitk::UuidProvider,
    scroll_offsets: (i64, i64),
    dragging: (bool, bool),

    python: python::Python,
    content_ids: Option<[ContentId; 2]>,
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
        text_formatter: ConsoleTextFormatter { cached: None },
        ui_store: uitk::UiStore::new(),
        uuid_provider: UuidProvider::new(),
        scroll_offsets: (0, 0),
        dragging: (false, false),
        python: python::Python::new(),
        content_ids: None,
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
    let input_state = input_state.change_origin(&win_rect);

    let mut cursor = state.input_buffer.as_ref().len();
    uitk::string_input(
        &mut state.input_buffer,
        &input_state,
        false,
        &mut cursor,
        &mut state.uuid_provider,
    );

    let mut autoscroll = false;
    if check_enter_pressed(&input_state) && !state.input_buffer.as_ref().is_empty() {
        let cmd = state.input_buffer.as_ref().to_owned();
        let pyres = state.python.run_code(&cmd);
        state
            .console_buffer
            .mutate(&mut state.uuid_provider)
            .push(EvalResult { cmd, pyres });
        state.input_buffer.mutate(&mut state.uuid_provider).clear();
        autoscroll = true;
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

    let formatted = state.text_formatter.format(&state.input_buffer, &state.console_buffer, (win_w, win_h));

    let renderer = ConsoleCanvasRenderer { formatted };

    let time = guestlib::get_time();

    let mut uitk_context =
        state
            .ui_store
            .get_context(&mut framebuffer, &input_state, &mut state.uuid_provider, time);

    uitk_context.dyn_scrollable_canvas(
        &rect_console,
        &renderer,
        &mut state.scroll_offsets,
        &mut state.dragging,
    );

    if autoscroll {
        let max_h = renderer.formatted.as_ref().h;
        uitk::set_autoscroll(&rect_console, max_h, &mut state.scroll_offsets);
    }

    state.content_ids = Some([state.input_buffer.get_id(), state.console_buffer.get_id()]);
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


struct ConsoleTextFormatter {
    cached: Option<(ContentId, FormattedRichText)>,
}

impl ConsoleTextFormatter {

    fn format(
        &mut self,
        input_buffer: &TrackedContent<String>,
        console_buffer: &TrackedContent<Vec<EvalResult>>,
        win_shape: (u32, u32)
    ) -> TrackedContent<&FormattedRichText> {

        let (win_w, _win_h) = win_shape;

        let new_cid = ContentId::from_hash((
            input_buffer.get_id(),
            console_buffer.get_id(),
            win_w,
        ));

        // Clearing cache if we get a new content ID
        self.cached.take_if(|(cid, _)| *cid != new_cid);

        let (_, formatted) = self.cached.get_or_insert_with(|| {
            let console_rich_text = get_console_rich_text(input_buffer.as_ref(), console_buffer.as_ref());
            let formatted = format_rich_lines(&console_rich_text, win_w);
            (new_cid, formatted)
        });

        TrackedContent::new_with_id(formatted, new_cid)
    }   

}

struct ConsoleCanvasRenderer<'a> {
    formatted: TrackedContent<&'a FormattedRichText>,
}


impl<'a> uitk::TileRenderer for ConsoleCanvasRenderer<'a> {
    fn shape(&self) -> (u32, u32) {
        let FormattedRichText { w, h, .. } = **self.formatted.as_ref();
        (w, h)
    }

    fn content_id(&self, src_rect: &Rect) -> ContentId {
        ContentId::from_hash((
            self.formatted.get_id(),
            src_rect
        ))
    }

    fn render<F: FbViewMut>(&self, dst_fb: &mut F, src_rect: &Rect) {

        let Rect { x0: ox, y0: oy, .. } = *src_rect;

        // TODO
        if ox != 0 {
            unimplemented!()
        }

        let mut y = 0;
        for line in self.formatted.as_ref().lines.iter() {
            // Bounding box of line in source
            let line_rect = Rect {
                x0: 0,
                y0: y,
                w: line.w,
                h: line.h,
            };

            if src_rect.intersection(&line_rect).is_some() {
                draw_rich_slice(dst_fb, &line.chars, 0, y - oy);
            }

            y += line.h as i64;
        }
    }
}

fn get_console_rich_text(input_buffer: &String, console_buffer: &Vec<EvalResult>) -> RichText {
    let font = &HACK_15;

    let mut console_rich_text = RichText::new();

    for res in console_buffer.iter() {
        console_rich_text.add_part(">>> ", Color::YELLOW, font, None);
        console_rich_text.add_part(&res.cmd, Color::WHITE, font, None);

        let color = match &res.pyres {
            python::EvalResult::Failure(_) => Color::RED,
            _ => Color::WHITE,
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
