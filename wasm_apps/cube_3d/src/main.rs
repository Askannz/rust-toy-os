use core::cell::OnceCell;
use applib::{Color, Framebuffer, Rect};
use applib::ui::scrollable_canvas::scrollable_canvas;
use guestlib::FramebufferHandle;

mod drawing;
use drawing::{draw_scene, Scene, load_scene};

struct AppState<'a> {
    fb_handle: FramebufferHandle,
    render_fb: Framebuffer<'a>,
    render_rect: Rect,
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
    let win_rect = guestlib::get_win_rect();
    let fb_handle = guestlib::create_framebuffer(win_rect.w, win_rect.h);
    let state = AppState {
        fb_handle,
        render_fb: Framebuffer::new_owned(W as u32, H as u32),
        render_rect: win_rect.zero_origin(),
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

    let mut framebuffer = guestlib::get_framebuffer(&mut state.fb_handle);

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
    scrollable_canvas(
        &mut framebuffer,
        &win_rect.zero_origin(),
        &state.render_fb,
        &mut state.render_rect,
        &input_state_local,
        &mut state.dragging_sbar
    );
}
