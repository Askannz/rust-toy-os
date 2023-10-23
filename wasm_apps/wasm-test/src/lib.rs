#![no_std]

extern crate alloc;

use core::panic::PanicInfo;
use core::cell::OnceCell;
use alloc::{string::String, format, boxed::Box};

#[global_allocator]
static ALLOC: wee_alloc::WeeAlloc = wee_alloc::WeeAlloc::INIT;

extern "C" {
    fn host_print_console(addr: i32, len: i32);
}

fn print_console(s: String) {
    let buf = s.as_bytes();
    let addr = buf.as_ptr() as i32;
    let len = buf.len() as i32;
    unsafe { host_print_console(addr, len) };
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

#[derive(Debug)]
#[repr(C)]
struct AppHandle {
    n: u32
}


//
//  END OF BOILERPLATE
//


#[derive(Debug)]
struct AppState {
    handle_ptr: *mut AppHandle,
    s: String,
    n: u64
}

static mut APP_STATE: OnceCell<AppState> = OnceCell::new();

#[no_mangle]
pub fn init() -> i32 {

    let handle: AppHandle = unsafe { core::mem::zeroed() };
    let handle = Box::new(handle);

    let handle_ptr = Box::leak(handle) as *mut AppHandle;
    let handle_addr = handle_ptr  as i32;

    let state = AppState {
        handle_ptr,
        s: "aaaa".into(),
        n: 0,
    };

    unsafe { 
        APP_STATE
            .set(state)
            .expect("Application was already initialized");
    };

    handle_addr
}

#[no_mangle]
pub fn step() {

    let state = unsafe { 
        APP_STATE
            .get_mut()
            .expect("Application is not initialized")
    };

    state.n += 1;
    let handle = unsafe { core::ptr::read_volatile(state.handle_ptr) };

    println!("{:?} {:?}", state, handle);
}
