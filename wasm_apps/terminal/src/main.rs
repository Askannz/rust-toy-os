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
use applib::drawing::text::{format_rich_lines, draw_rich_slice, Font, FormattedRichText, RichText, HACK_15};
use applib::uitk::{self, TileRenderer, TrackedContent};
use applib::drawing::primitives::draw_rect;

mod python;

#[derive(Debug)]
struct EvalResult {
    cmd: String,
    pyres: python::EvalResult,
}

static LOGGER: WasmLogger = WasmLogger;
const LOGGING_LEVEL: log::LevelFilter = log::LevelFilter::Debug;

struct AppState {
    fb_handle: FramebufferHandle,
    input_buffer: TrackedContent<String>,
    console_buffer: TrackedContent<Vec<EvalResult>>,

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

    let state = AppState { 
        fb_handle,
        input_buffer: TrackedContent::new(String::new()),
        console_buffer: TrackedContent::new(Vec::new()),
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

    let mut cursor = state.input_buffer.as_ref().len();
    uitk::string_input(&mut state.input_buffer, &mut state.caps, &input_state, false, &mut cursor);

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

    let console_rich_text = render_console(state.input_buffer.as_ref(), state.console_buffer.as_ref());
    let formatted = format_rich_lines(&console_rich_text, win_w);

    let renderer = ConsoleRenderer { formatted };

    framebuffer.fill(Color::BLACK);

    uitk::dyn_scrollable_canvas(
        &mut framebuffer,
        &rect_console,
        &renderer,
        &mut state.scroll_offsets,
        &input_state,
        &mut state.dragging,
    );

    if autoscroll {
        let (_max_w, max_h) = renderer.shape();
        uitk::set_autoscroll(&rect_console, max_h, &mut state.scroll_offsets);
    }

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

struct ConsoleRenderer {
    formatted: FormattedRichText,
}

impl uitk::TileRenderer for ConsoleRenderer {

    fn shape(&self) -> (u32, u32) {
       let FormattedRichText { w, h, .. } = self.formatted;
       (w, h)
    }

    fn render(&self, context: &mut uitk::TileRenderContext) {

        let uitk::TileRenderContext { dst_fb, dst_rect, src_rect, .. } = context;

        let Rect { x0: dst_x0, y0: dst_y0, h: dst_h, w: dst_w } = *dst_rect;
        let Rect { x0: ox, y0: oy, .. } = *src_rect;
    
        let src_rect = Rect { x0: *ox, y0: *oy, w: *dst_w, h: *dst_h };
    
        let mut y = 0;
        for line in self.formatted.lines.iter() {
    
            if y >= src_rect.y0 && y + (line.h as i64) <= src_rect.y0 + (src_rect.h as i64) {
                draw_rich_slice(dst_fb, &line.chars, *dst_x0, dst_y0 + y - oy);
            }
            
            y += line.h as i64;
        }

    }
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
