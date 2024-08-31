use core::cell::OnceCell;
use applib::{Color, Framebuffer, Rect, OwnedPixels, FbViewMut};
use applib::uitk;
use guestlib::FramebufferHandle;
use guestlib::{WasmLogger};

mod drawing;
use drawing::{draw_scene, Scene, load_scene};

static LOGGER: WasmLogger = WasmLogger;
const LOGGING_LEVEL: log::LevelFilter = log::LevelFilter::Debug;

struct AppState {
    fb_handle: FramebufferHandle,
    render_fb: Framebuffer<OwnedPixels>,
    scroll_offsets: (i64, i64),
    dragging_sbar: bool,
    prev_pointer: Option<(i64, i64)>,
    scene: Scene,
}

static mut APP_STATE: OnceCell<AppState> = OnceCell::new();

const W: usize = 400;
const H: usize = 400;

fn main() {}

#[no_mangle]
pub fn init() -> () {

    log::set_max_level(LOGGING_LEVEL);
    log::set_logger(&LOGGER).unwrap();

    let win_rect = guestlib::get_win_rect();
    let fb_handle = guestlib::create_framebuffer(win_rect.w, win_rect.h);
    let state = AppState {
        fb_handle,
        render_fb: Framebuffer::new_owned(W as u32, H as u32),
        scroll_offsets: (0, 0),
        dragging_sbar: false,
        prev_pointer: None,
        scene: load_scene()
    };
    unsafe { APP_STATE.set(state).unwrap_or_else(|_| panic!("App already initialized")); }
}

#[no_mangle]
pub fn step() {

    let state = unsafe { APP_STATE.get_mut().expect("App not initialized") };

    let win_rect = guestlib::get_win_rect();
    let system_state = guestlib::get_system_state();

    let mut framebuffer = state.fb_handle.as_framebuffer();

    let input_state_local = system_state.input.change_origin(&win_rect);
    let pointer = &input_state_local.pointer;

    let redraw = match state.prev_pointer {
        Some((px, py)) if (pointer.x, pointer.y) == (px, py) => false,
        _ => {
            state.prev_pointer = Some((pointer.x, pointer.y));
            true
        },
    };

    if redraw {
        let xf = (pointer.x as f32) / ((W - 1) as f32);
        let yf = (pointer.y as f32) / ((H - 1) as f32);
        state.render_fb.fill(Color::WHITE);
        draw_scene(&mut state.render_fb, &state.scene, xf, yf);
    }

    framebuffer.fill(Color::BLACK);
    uitk::scrollable_canvas(
        &mut framebuffer,
        &win_rect.zero_origin(),
        &state.render_fb,
        &mut state.scroll_offsets,
        &input_state_local,
        &mut state.dragging_sbar
    );
}
