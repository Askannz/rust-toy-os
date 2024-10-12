#![no_std]
extern crate alloc;

use alloc::format;
use alloc::vec;
use alloc::vec::Vec;
use applib::{input::InputState, BorrowedMutPixels, Framebuffer, Rect};
use core::fmt::Debug;
use core::mem::size_of;
use log::{Log, Metadata, Record};

#[global_allocator]
static ALLOC: dlmalloc::GlobalDlmalloc = dlmalloc::GlobalDlmalloc;

extern "C" {

    fn host_print_console(addr: i32, len: i32);
    fn host_log(addr: i32, len: i32, level: i32);
    fn host_get_input_state(addr: i32);
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
struct FramebufferHandle {
    ptr: *mut u32,
    w: u32,
    h: u32,
}

impl FramebufferHandle {
    fn new(w: u32, h: u32) -> Self {
        let ptr = vec![0u32; (w * h) as usize].leak().as_mut_ptr();
        Self { ptr, w, h }
    }

    fn as_framebuffer(&mut self) -> Framebuffer<BorrowedMutPixels> {
        let FramebufferHandle { ptr, w, h } = *self;

        let fb_data = unsafe { core::slice::from_raw_parts_mut(ptr, (w * h) as usize) };

        Framebuffer::<BorrowedMutPixels>::new(fb_data, w, h)
    }

    fn register(&self) {
        unsafe { host_set_framebuffer(self.ptr as i32, self.w as i32, self.h as i32) };
    }

    fn destroy(self) {
        let n = (self.w * self.h) as usize;
        let data = unsafe { Vec::from_raw_parts(self.ptr, n, n) };
        core::mem::drop(data)
    }
}

pub fn print_console(s: &str) {
    let buf = s.as_bytes();
    let addr = buf.as_ptr() as i32;
    let len = buf.len() as i32;
    unsafe { host_print_console(addr, len) };
}

pub fn get_input_state() -> InputState {
    let mut buf = [0u8; size_of::<InputState>()];
    let addr = buf.as_mut_ptr() as i32;
    unsafe {
        host_get_input_state(addr);
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

pub struct PixelData {
    fb_handle: FramebufferHandle,
}

impl PixelData {
    pub fn new() -> Self {
        let Rect { w, h, .. } = get_win_rect();
        let fb_handle = FramebufferHandle::new(w, h);
        fb_handle.register();
        PixelData { fb_handle }
    }

    pub fn get_framebuffer(&mut self) -> Framebuffer<BorrowedMutPixels> {
        let Rect { w, h, .. } = get_win_rect();

        let fb_w = self.fb_handle.w;
        let fb_h = self.fb_handle.h;

        if (fb_w, fb_h) != (w, h) {
            let new_fb_handle = FramebufferHandle::new(w, h);
            new_fb_handle.register();
            let old_fb_handle = core::mem::replace(&mut self.fb_handle, new_fb_handle);
            old_fb_handle.destroy();
        }

        self.fb_handle.as_framebuffer()
    }
}

fn register_framebuffer_data(data: Vec<u32>, w: u32, h: u32) -> *mut u32 {
    assert_eq!(data.len(), (w * h) as usize);
    let ptr = vec![0u32; (w * h) as usize].leak().as_mut_ptr();
    unsafe { host_set_framebuffer(ptr as i32, w as i32, h as i32) };
    ptr
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

    unsafe {
        host_get_time(ptr as i32);
    }

    let s: &[u8; 8] = unsafe { core::slice::from_raw_parts(ptr, 8).try_into().unwrap() };

    f64::from_le_bytes(*s)
}

pub fn get_consumed_fuel() -> u64 {
    let ptr = vec![0u8; 8].leak().as_mut_ptr();

    unsafe {
        host_get_consumed_fuel(ptr as i32);
    }

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
    }};
}

#[macro_export]
macro_rules! measure_time {
    ($key:expr, $block:expr) => {{
        let t0 = guestlib::get_time();
        let retval = { $block };
        let t1 = guestlib::get_time();

        let elapsed = t1 - t0;
        guestlib::print_console(&format!("{}: {:.2}ms", $key, elapsed));
        retval
    }};
}

pub fn save_timing(key: &str, consumed: u64) {
    let key_buf = key.as_bytes();
    let key_addr = key_buf.as_ptr() as i32;
    let key_len = key_buf.len() as i32;

    let consumed_buf = consumed.to_le_bytes();
    let consumed_addr = consumed_buf.as_ptr() as i32;

    unsafe {
        host_save_timing(key_addr, key_len, consumed_addr);
    }
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

        let s = format!("{} -- {}", record.module_path().unwrap(), record.args(),);

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
