#![no_std]
extern crate alloc;


use core::fmt::Debug;
use core::mem::size_of;
use alloc::vec;
use alloc::format;
use log::{Log, Metadata, Record};
use applib::{SystemState, Framebuffer, Rect, BorrowedMutPixels};

#[global_allocator]
static ALLOC: dlmalloc::GlobalDlmalloc = dlmalloc::GlobalDlmalloc;

extern "C" {

    fn host_print_console(addr: i32, len: i32);
    fn host_log(addr: i32, len: i32, level: i32);
    fn host_get_system_state(addr: i32);
    fn host_get_win_rect(addr: i32);
    fn host_set_framebuffer(addr: i32, w: i32, h: i32);

    fn host_tcp_connect(ip_addr: i32, port: i32) -> i32;
    fn host_tcp_may_send(handle_id: i32) -> i32;
    fn host_tcp_may_recv(handle_id: i32) -> i32;
    fn host_tcp_write(addr: i32, len: i32, handle_id: i32) -> i32;
    fn host_tcp_read(addr: i32, len: i32, handle_id: i32) -> i32;
    fn host_tcp_close(handle_id: i32);
    fn host_get_time(buf: i32);

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

impl FramebufferHandle {
    pub fn as_framebuffer(&mut self) -> Framebuffer<BorrowedMutPixels> {
        let FramebufferHandle { framebuffer_ptr, w, h } = *self;

        let fb_data = unsafe {
            core::slice::from_raw_parts_mut(framebuffer_ptr, (w*h) as usize)
        };

        Framebuffer::<BorrowedMutPixels>::new(fb_data, w, h)
    }
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

pub fn tcp_connect(ip_addr: [u8; 4], port: u16) -> anyhow::Result<i32> {

    let ip_addr: i32 = i32::from_le_bytes(ip_addr);
    let port: i32 = port.into();
    let retval = unsafe { host_tcp_connect(ip_addr, port) };
    
    if retval < 0 {
        Err(anyhow::Error::msg("TCP connect failed"))
    } else {
        let handle_id = retval;
        Ok(handle_id)
    }
}

pub fn tcp_may_send(handle_id: i32) -> bool {
    unsafe { host_tcp_may_send(handle_id) != 0 }
}

pub fn tcp_may_recv(handle_id: i32) -> bool {
    unsafe { host_tcp_may_recv(handle_id) != 0 }
}

pub fn tcp_write(buf: &[u8], handle_id: i32) -> anyhow::Result<usize> {

    let retval = unsafe {
        let addr = buf.as_ptr() as i32;
        let len = buf.len() as i32;
        host_tcp_write(addr, len, handle_id)
    };

    if retval < 0 {
        Err(anyhow::Error::msg("TCP write failed"))
    } else {
        let written_len = retval.try_into().map_err(anyhow::Error::msg)?;
        Ok(written_len)
    }
}

pub fn tcp_read(buf: &mut [u8], handle_id: i32) -> anyhow::Result<usize> {

    let retval = unsafe {
        let addr = buf.as_ptr() as i32;
        let len = buf.len() as i32;
        host_tcp_read(addr, len, handle_id)
    };

    if retval < 0 {
        Err(anyhow::Error::msg("TCP read failed"))
    } else {
        let read_len = retval.try_into().map_err(anyhow::Error::msg)?;
        Ok(read_len)
    }
}

pub fn tcp_close(handle_id: i32) {
    unsafe { host_tcp_close(handle_id) }
}

pub fn get_time() -> f64 {
    let ptr = vec![0u8; 8].leak().as_mut_ptr();

    unsafe { host_get_time(ptr as i32); }
    
    let s: &[u8; 8] = unsafe { core::slice::from_raw_parts(ptr, 8).try_into().unwrap() };

    f64::from_le_bytes(*s)
}


pub fn get_consumed_fuel() -> u64 {

    let ptr = vec![0u8; 8].leak().as_mut_ptr();

    unsafe { host_get_consumed_fuel(ptr as i32); }
    
    let s: &[u8; 8] = unsafe { core::slice::from_raw_parts(ptr, 8).try_into().unwrap() };

    u64::from_le_bytes(*s)
}

#[macro_export]
macro_rules! measure_fuel {
    ($key:expr, $block:expr) => {{

        let u0 = guestlib::get_consumed_fuel();
        let retval = { $block };
        let u1 = guestlib::get_consumed_fuel();

        let consumed = u1 - u0;
        guestlib::save_timing($key, consumed);
        retval
    }}
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


pub struct WasmLogger;


impl Log for WasmLogger {

    // TODO
    fn enabled(&self, _metadata: &Metadata<'_>) -> bool {
        true
    }

    fn log(&self, record: &Record<'_>) {

        if !self.enabled(record.metadata()) {
            return;
        }

        let s = format!(
            "{} -- {}",
            record.module_path().unwrap(),
            record.args(),
        );
 
        let buf = s.as_bytes();
        let addr = buf.as_ptr() as i32;
        let len = buf.len() as i32;
        let level = record.level() as i32;

        unsafe { host_log(addr, len, level) };
    }

    fn flush(&self) {}
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
