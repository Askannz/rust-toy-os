#[derive(Debug, Clone, Copy, enumn::N)]
#[repr(u16)]
pub enum EventType {
    EV_SYN = 0x0,
    EV_KEY = 0x1,
    EV_REL = 0x2,
}

pub const MAX_KEYCODES: usize = 255;

#[derive(Debug, Clone, Copy, enumn::N)]
#[repr(u16)]
pub enum Keycode {
    BTN_MOUSE = 272,
    KEY_Q = 16,
    KEY_W = 17,
    KEY_E = 18,
    KEY_LEFTSHIFT = 42,
}
