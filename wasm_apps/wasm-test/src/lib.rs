const DATA: &'static str = "WASSUP";

extern "C" {
    fn printouille();
}

#[no_mangle]
pub fn hello() -> i32 {
    unsafe { printouille() };
    DATA as *const str as *const u8 as i32
}