use applib::uitk::{UiStore, UuidProvider};
use applib::{Color, FbViewMut, Framebuffer, OwnedPixels};
use core::cell::OnceCell;
use guestlib::PixelData;
use guestlib::WasmLogger;

mod drawing;
use drawing::{draw_scene, load_scene, Scene};

static LOGGER: WasmLogger = WasmLogger;
const LOGGING_LEVEL: log::LevelFilter = log::LevelFilter::Debug;

struct AppState {
    pixel_data: PixelData,
    render_fb: Framebuffer<OwnedPixels>,
    ui_store: UiStore,
    uuid_provider: UuidProvider,
    scroll_offsets: (i64, i64),
    dragging_sbar: (bool, bool),
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

    let state = AppState {
        pixel_data: PixelData::new(),
        render_fb: Framebuffer::new_owned(W as u32, H as u32),
        ui_store: UiStore::new(),
        uuid_provider: UuidProvider::new(),
        scroll_offsets: (0, 0),
        dragging_sbar: (false, false),
        prev_pointer: None,
        scene: load_scene(),
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

    let win_rect = guestlib::get_win_rect();
    let input_state = guestlib::get_input_state();

    let mut framebuffer = state.pixel_data.get_framebuffer();

    let input_state_local = input_state.change_origin(&win_rect);
    let pointer = &input_state_local.pointer;

    let redraw = match state.prev_pointer {
        Some((px, py)) if (pointer.x, pointer.y) == (px, py) => false,
        _ => {
            state.prev_pointer = Some((pointer.x, pointer.y));
            true
        }
    };

    if redraw {
        let xf = (pointer.x as f32) / ((W - 1) as f32);
        let yf = (pointer.y as f32) / ((H - 1) as f32);
        state.render_fb.fill(Color::WHITE);
        draw_scene(&mut state.render_fb, &state.scene, xf, yf);
    }

    framebuffer.fill(Color::BLACK);

    let mut uitk_context = state.ui_store.get_context(
        &mut framebuffer,
        &input_state_local,
        &mut state.uuid_provider,
    );

    // uitk_context.scrollable_canvas(
    //     &win_rect.zero_origin(),
    //     &state.render_fb,
    //     &mut state.scroll_offsets,
    //     &mut state.dragging_sbar,
    // );
}
