#![no_std]
extern crate alloc;


use core::fmt::Debug;
use core::mem::size_of;
use alloc::{vec};
use applib::{SystemState, Framebuffer, Rect};

#[global_allocator]
static ALLOC: dlmalloc::GlobalDlmalloc = dlmalloc::GlobalDlmalloc;

extern "C" {

    fn host_print_console(addr: i32, len: i32);
    fn host_get_system_state(addr: i32);
    fn host_get_win_rect(addr: i32);
    fn host_set_framebuffer(addr: i32, w: i32, h: i32);

    fn host_tcp_connect(ip_addr: i32, port: i32) -> i32;
    fn host_tcp_may_send(handle_id: i32) -> i32;
    fn host_tcp_may_recv(handle_id: i32) -> i32;
    fn host_tcp_write(addr: i32, len: i32, handle_id: i32) -> i32;
    fn host_tcp_read(addr: i32, len: i32, handle_id: i32) -> i32;
    fn host_tcp_close(handle_id: i32);

    fn host_get_consumed_fuel(addr: i32);
    fn host_save_timing(key_addr: i32, key_len: i32, consumed_addr: i32);

    fn host_qemu_dump(addr: i32, len: i32);
}


#[derive(Debug)]
pub struct FramebufferHandle {
    framebuffer_ptr: *mut u32,
    w: u32,
    h: u32,
}

pub fn print_console(s: &str) {
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

pub fn get_win_rect() -> Rect {
    let mut buf = [0u8; size_of::<Rect>()];
    let addr = buf.as_mut_ptr() as i32;
    unsafe { 
        host_get_win_rect(addr);
        core::mem::transmute(buf)
    }
}

pub fn create_framebuffer(w: u32, h: u32) -> FramebufferHandle {
    let ptr = vec![0u32; (w*h) as usize].leak().as_mut_ptr();
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
        core::slice::from_raw_parts_mut(framebuffer_ptr, (w*h) as usize)
    };

    Framebuffer::new(fb_data, w, h)
}


pub fn tcp_connect(ip_addr: [u8; 4], port: u16) -> i32{
    let ip_addr: i32 = i32::from_le_bytes(ip_addr);
    let port: i32 = port.into();
    let handle_id = unsafe { host_tcp_connect(ip_addr, port) };
    handle_id
}

pub fn tcp_may_send(handle_id: i32) -> bool {
    unsafe { host_tcp_may_send(handle_id) != 0 }
}

pub fn tcp_may_recv(handle_id: i32) -> bool {
    unsafe { host_tcp_may_recv(handle_id) != 0 }
}

pub fn tcp_write(buf: &[u8], handle_id: i32) -> usize {
    unsafe {
        let addr = buf.as_ptr() as i32;
        let len = buf.len() as i32;
        let written_len = host_tcp_write(addr, len, handle_id);
        written_len.try_into().unwrap()
    }
}

pub fn tcp_read(buf: &mut [u8], handle_id: i32) -> usize {
    unsafe {
        let addr = buf.as_ptr() as i32;
        let len = buf.len() as i32;
        let read_len = host_tcp_read(addr, len, handle_id);
        read_len.try_into().unwrap()
    }
}

pub fn tcp_close(handle_id: i32) {
    unsafe { host_tcp_close(handle_id) }
}

pub fn get_consumed_fuel() -> u64 {

    let ptr = vec![0u8; 8].leak().as_mut_ptr();

    unsafe { host_get_consumed_fuel(ptr as i32); }
    
    let s: &[u8; 8] = unsafe { core::slice::from_raw_parts(ptr, 8).try_into().unwrap() };

    u64::from_le_bytes(*s)
}

pub fn save_timing(key: &str, consumed: u64) {

    let key_buf = key.as_bytes();
    let key_addr = key_buf.as_ptr() as i32;
    let key_len = key_buf.len() as i32;

    let consumed_buf = consumed.to_le_bytes();
    let consumed_addr = consumed_buf.as_ptr() as i32;

    unsafe { host_save_timing(key_addr, key_len, consumed_addr); }
}

pub fn qemu_dump(buf: &[u8]) {
    let addr = buf.as_ptr() as i32;
    let len = buf.len() as i32;
    unsafe { host_qemu_dump(addr, len) };
}

// #[macro_export]
// macro_rules! print {
//     ($($arg:tt)*) => {
//         $crate::print_console(&format!($($arg)*))
//     };
// }

// #[macro_export]
// macro_rules! println {
//     () => (print!("\n"));
//     ($fmt:expr) => ($crate::print!(concat!($fmt, "\n")));
//     ($fmt:expr, $($arg:tt)*) => ($crate::print!(concat!($fmt, "\n"), $($arg)*));
// }

// #[panic_handler]
// fn panic(info: &PanicInfo) ->  ! {
//     println!("{}", info);
//     loop {}
// }
