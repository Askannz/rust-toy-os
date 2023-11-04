#![no_std]

extern crate alloc;

use core::panic::PanicInfo;
use core::cell::OnceCell;
use core::mem::size_of;
use alloc::{string::String, format};
use alloc::vec;

use applib::{SystemState, Framebuffer, Color};

mod drawing;

use drawing::draw_cube;

#[global_allocator]
static ALLOC: wee_alloc::WeeAlloc = wee_alloc::WeeAlloc::INIT;

extern "C" {
    fn host_print_console(addr: i32, len: i32);
    fn host_get_system_state(addr: i32);
    fn host_set_framebuffer(addr: i32, w: i32, h: i32);
}

fn print_console(s: String) {
    let buf = s.as_bytes();
    let addr = buf.as_ptr() as i32;
    let len = buf.len() as i32;
    unsafe { host_print_console(addr, len) };
}

fn get_system_state() -> SystemState {
    let mut buf = [0u8; size_of::<SystemState>()];
    let addr = buf.as_mut_ptr() as i32;
    unsafe { 
        host_get_system_state(addr);
        core::mem::transmute(buf)
    }
}

macro_rules! print {
    ($($arg:tt)*) => {
        print_console(format!($($arg)*))
    };
}

macro_rules! println {
    () => (print!("\n"));
    ($fmt:expr) => (print!(concat!($fmt, "\n")));
    ($fmt:expr, $($arg:tt)*) => (print!(concat!($fmt, "\n"), $($arg)*));
}

#[panic_handler]
fn panic(info: &PanicInfo) ->  ! {
    println!("{}", info);
    loop {}
}


//
//  END OF BOILERPLATE
//

#[derive(Debug)]
struct AppState {
    framebuffer_ptr: *mut u8,
    w: usize,
    h: usize,
}

static mut APP_STATE: OnceCell<AppState> = OnceCell::new();

const W: usize = 400;
const H: usize = 400;

#[no_mangle]
pub fn init() -> () {

    let framebuffer_ptr = unsafe {
        let ptr = vec![128u8; W*H*4].leak().as_mut_ptr();
        host_set_framebuffer(ptr as i32, W as i32, H as i32);
        ptr
    };

    let state = AppState {
        framebuffer_ptr,
        w: W,
        h: H,
    };

    unsafe { 
        APP_STATE
            .set(state)
            .expect("Application was already initialized");
    };
}

#[no_mangle]
pub fn step() {

    let state = unsafe { 
        APP_STATE
            .get_mut()
            .expect("Application is not initialized")
    };

    let system_state = get_system_state();

    let mut framebuffer = unsafe {

        let AppState { framebuffer_ptr, w, h } = *state;
        let fb_data = core::slice::from_raw_parts_mut(framebuffer_ptr, w*h*4);

        Framebuffer  {
            data: fb_data,
            w: w as i32,
            h: h as i32,
        }
    };

    let pointer = system_state.pointer;
    let xf = (pointer.x as f32) / ((W - 1) as f32);
    let yf = (pointer.y as f32) / ((H - 1) as f32);

    let mut region = framebuffer.as_region();
    region.fill(&Color(0, 0, 0));
    draw_cube(&mut region, xf, yf)

    //println!("{:?} {:?}", state, system_state);
}
