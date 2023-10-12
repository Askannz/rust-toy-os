#![no_std]

extern crate alloc;

use core::panic::PanicInfo;
use alloc::{string::String, format, vec::Vec};

#[global_allocator]
static ALLOC: wee_alloc::WeeAlloc = wee_alloc::WeeAlloc::INIT;

#[derive(Debug)]
#[repr(C)]
struct AppData {
    n: u32
}

extern "C" {
    fn host_print_console(addr: i32, len: i32);
    fn host_get_appdata(addr: i32);
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

fn get_appdata() -> AppData {
    let mut data: AppData = unsafe { 
        core::mem::zeroed()
    };
    let addr = (&mut data) as *mut AppData as i32;
    unsafe { host_get_appdata(addr) };
    data
}

#[no_mangle]
pub fn hello() -> i32 {

    let app_data = get_appdata();
    println!("Hello from WASM, {:?}", app_data);

    42
}

#[panic_handler]
fn panic(info: &PanicInfo) ->  ! {
    println!("{}", info);
    loop {}
}
