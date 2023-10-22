#![no_std]

extern crate alloc;

use core::panic::PanicInfo;
use core::cell::OnceCell;
use alloc::{string::String, format};

#[global_allocator]
static ALLOC: wee_alloc::WeeAlloc = wee_alloc::WeeAlloc::INIT;

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

#[panic_handler]
fn panic(info: &PanicInfo) ->  ! {
    println!("{}", info);
    loop {}
}


trait Application {
    fn new() -> Self;
    fn step(&mut self);
}

#[derive(Debug)]
#[repr(C)]
struct AppData {
    n: u32
}

fn get_appdata() -> AppData {
    let mut data: AppData = unsafe { 
        core::mem::zeroed()
    };
    let addr = (&mut data) as *mut AppData as i32;
    unsafe { host_get_appdata(addr) };
    data
}


//
//  END OF BOILERPLATE
//


#[derive(Debug)]
struct TestApp {
    s: String,
    n: u64
}

static mut LOCAL_DATA: OnceCell<TestApp> = OnceCell::new();

impl Application for TestApp {

    fn new() -> Self {
        TestApp { s: "Pouet".into(), n: 0 }
    }

    fn step(&mut self) {
        println!("App state: {:?}", self);
        self.n += 1;
    }
}

#[no_mangle]
pub fn init() {
    unsafe { 
        let app = TestApp::new();
        LOCAL_DATA.set(app).expect("Application was already initialized")
    };
}

#[no_mangle]
pub fn step() {
    unsafe { 
        let app = LOCAL_DATA.get_mut().expect("Application is not initialized");
        app.step();
    }
}
