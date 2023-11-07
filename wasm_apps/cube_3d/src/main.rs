#![no_std]
#![no_main]

use core::cell::OnceCell;
use guestlib::FramebufferHandle;

mod drawing;
use drawing::draw_cube;

#[derive(Debug)]
struct AppState {
    fb_handle: FramebufferHandle
}

static mut APP_STATE: OnceCell<AppState> = OnceCell::new();

const W: usize = 200;
const H: usize = 200;

#[no_mangle]
pub fn init() -> () {
    let fb_handle = guestlib::create_framebuffer(W, H);
    let state = AppState { fb_handle };
    unsafe { APP_STATE.set(state).expect("App already initialized"); }
}

#[no_mangle]
pub fn step() {

    let state = unsafe { APP_STATE.get_mut().expect("App not initialized") };

    let system_state = guestlib::get_system_state();

    let mut framebuffer = guestlib::get_framebuffer(&mut state.fb_handle);

    let pointer = system_state.pointer;
    let xf = (pointer.x as f32) / ((W - 1) as f32);
    let yf = (pointer.y as f32) / ((H - 1) as f32);

    framebuffer.fill(&[0u8; 4]);
    draw_cube(&mut framebuffer, xf, yf);
}
