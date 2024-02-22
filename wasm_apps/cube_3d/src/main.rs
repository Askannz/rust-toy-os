use core::cell::OnceCell;
use applib::Color;
use guestlib::FramebufferHandle;

mod drawing;
use drawing::{draw_scene, Scene, load_scene};

#[derive(Debug)]
struct AppState {
    fb_handle: FramebufferHandle,
    scene: Scene,
}

static mut APP_STATE: OnceCell<AppState> = OnceCell::new();

const W: usize = 200;
const H: usize = 200;

fn main() {}

#[no_mangle]
pub fn init() -> () {
    let win_rect = guestlib::get_win_rect();
    let fb_handle = guestlib::create_framebuffer(win_rect.w, win_rect.h);
    let state = AppState { fb_handle, scene: load_scene() };
    unsafe { APP_STATE.set(state).expect("App already initialized"); }
}

#[no_mangle]
pub fn step() {

    let state = unsafe { APP_STATE.get_mut().expect("App not initialized") };

    let win_rect = guestlib::get_win_rect();
    let system_state = guestlib::get_system_state();

    let mut framebuffer = guestlib::get_framebuffer(&mut state.fb_handle);

    let input_state_local = system_state.input.change_origin(&win_rect);
    let pointer = input_state_local.pointer;
    let xf = (pointer.x as f32) / ((W - 1) as f32);
    let yf = (pointer.y as f32) / ((H - 1) as f32);

    framebuffer.fill(Color::BLACK);
    draw_scene(&mut framebuffer, &state.scene, xf, yf);
}
