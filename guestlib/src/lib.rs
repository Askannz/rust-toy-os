#![no_std]
extern crate alloc;

use core::panic::PanicInfo;
use core::fmt::Debug;
use core::mem::size_of;
use alloc::{string::String, format, vec};
use applib::{SystemState, Framebuffer};

#[global_allocator]
static ALLOC: wee_alloc::WeeAlloc = wee_alloc::WeeAlloc::INIT;

extern "C" {
    fn host_print_console(addr: i32, len: i32);
    fn host_get_system_state(addr: i32);
    fn host_set_framebuffer(addr: i32, w: i32, h: i32);
}


#[derive(Debug)]
pub struct FramebufferHandle {
    framebuffer_ptr: *mut u8,
    w: usize,
    h: usize,
}

pub fn print_console(s: String) {
    let buf = s.as_bytes();
    let addr = buf.as_ptr() as i32;
    let len = buf.len() as i32;
    unsafe { host_print_console(addr, len) };
}

pub fn get_system_state() -> SystemState {
    let mut buf = [0u8; size_of::<SystemState>()];
    let addr = buf.as_mut_ptr() as i32;
    unsafe { 
        host_get_system_state(addr);
        core::mem::transmute(buf)
    }
}

pub fn create_framebuffer(w: usize, h: usize) -> FramebufferHandle {
    let ptr = vec![128u8; w*h*4].leak().as_mut_ptr();
    unsafe { host_set_framebuffer(ptr as i32, w as i32, h as i32) };
    FramebufferHandle {
        framebuffer_ptr: ptr,
        w,
        h,
    }
}

pub fn get_framebuffer<'a>(handle: &'a mut FramebufferHandle) -> Framebuffer<'a> {

    let FramebufferHandle { framebuffer_ptr, w, h } = *handle;

    let fb_data = unsafe {
        core::slice::from_raw_parts_mut(framebuffer_ptr, w*h*4)
    };

    Framebuffer::new(fb_data, w, h)
}

#[macro_export]
macro_rules! print {
    ($($arg:tt)*) => {
        $crate::print_console(format!($($arg)*))
    };
}

#[macro_export]
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
