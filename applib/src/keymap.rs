#[derive(PartialEq, Eq, Debug, Clone, Copy, enumn::N)]
#[repr(u16)]
#[allow(non_camel_case_types)]
pub enum EventType {
    EV_SYN = 0x0,
    EV_KEY = 0x1,
    EV_REL = 0x2,
}

#[derive(PartialEq, Eq, Debug, Clone, Copy, enumn::N)]
#[repr(u16)]
#[allow(non_camel_case_types)]
pub enum Keycode {
    BTN_MOUSE = 272,
    KEY_Q = 16,
    KEY_W = 17,
    KEY_E = 18,
    KEY_LEFTSHIFT = 42,
}

