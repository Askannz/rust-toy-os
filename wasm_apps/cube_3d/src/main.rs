use applib::input::PointerState;
use applib::uitk::{ContentId, TileRenderer, UiContext, UiStore, UuidProvider};
use applib::{Color, FbViewMut, Framebuffer, OwnedPixels, Rect};
use applib::content::TrackedContent;
use core::cell::OnceCell;
use guestlib::PixelData;
use guestlib::WasmLogger;

mod drawing;
use drawing::{draw_scene, load_scene, Scene};

static LOGGER: WasmLogger = WasmLogger;
const LOGGING_LEVEL: log::LevelFilter = log::LevelFilter::Debug;

struct AppState {
    pixel_data: PixelData,
    ui_store: UiStore,
    uuid_provider: UuidProvider,
    scroll_offsets: (i64, i64),
    dragging_sbar: (bool, bool),
    scene: Scene,
}

static mut APP_STATE: OnceCell<AppState> = OnceCell::new();

const W: u32 = 400;
const H: u32 = 400;

fn main() {}

#[no_mangle]
pub fn init() -> () {
    log::set_max_level(LOGGING_LEVEL);
    log::set_logger(&LOGGER).unwrap();

    let uuid_provider = UuidProvider::new();

    let state = AppState {
        pixel_data: PixelData::new(),
        ui_store: UiStore::new(),
        uuid_provider,
        scroll_offsets: (0, 0),
        dragging_sbar: (false, false),
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
    let stylesheet = guestlib::get_stylesheet();

    let time = guestlib::get_time();

    let mut framebuffer = state.pixel_data.get_framebuffer();

    let mut uitk_context = state.ui_store.get_context(
        &mut framebuffer,
        &stylesheet,
        &input_state,
        &mut state.uuid_provider,
        time,
    );

    let renderer = SceneRenderer {
        canvas_shape: win_rect.shape(),
        scene: &state.scene,
        pointer: &input_state.pointer,
        bg_color: stylesheet.colors.element,
    };

    uitk_context.dynamic_canvas(
        &win_rect.zero_origin(),
        &renderer,
        &mut state.scroll_offsets,
        &mut state.dragging_sbar,
    );
}


struct SceneRenderer<'a> {
    canvas_shape: (u32, u32),
    scene: &'a Scene,
    pointer: &'a PointerState,
    bg_color: Color,
}

impl<'a> TileRenderer for SceneRenderer<'a> {

    fn shape(&self) -> (u32, u32) {
        self.canvas_shape
    }

    fn tile_shape(&self) -> (u32, u32) {
        (W, H)
    }

    fn content_id(&self, viewport_rect: &Rect) -> ContentId {

        let Rect { x0, y0, .. } = *viewport_rect;

        let is_scene_tile = x0 == 0 && y0 == 0;

        // Technically depends on the scene too, but we assume it doesn´t change
        ContentId::from_hash(&(
            is_scene_tile,
            self.pointer.x,
            self.pointer.y,
        ))
    }

    fn render<F: FbViewMut>(&self, dst_fb: &mut F, viewport_rect: &Rect) {

        let Rect { x0, y0, w, h } = *viewport_rect;
        

        dst_fb.fill(self.bg_color);

        let is_scene_tile = x0 == 0 && y0 == 0;

        if is_scene_tile {
            assert_eq!((w, h), (W, H));
            let xf = (self.pointer.x as f32) / ((W - 1) as f32);
            let yf = (self.pointer.y as f32) / ((H - 1) as f32);
            draw_scene(dst_fb, self.scene, xf, yf);
        }
    }
}