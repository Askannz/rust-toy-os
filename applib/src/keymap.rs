use alloc::collections::BTreeMap;
use lazy_static::lazy_static;

#[derive(PartialEq, Eq, Debug, Clone, Copy, enumn::N)]
#[repr(u16)]
#[allow(non_camel_case_types)]
pub enum EventType {
    EV_SYN = 0x0,
    EV_KEY = 0x1,
    EV_REL = 0x2,
}

#[derive(PartialEq, Eq, PartialOrd, Ord, Debug, Clone, Copy, enumn::N)]
#[repr(u16)]
#[allow(non_camel_case_types)]
pub enum Keycode {
    BTN_MOUSE = 272,
    KEY_Q = 16,
    KEY_W = 17,
    KEY_E = 18,
    KEY_LEFTSHIFT = 42,
}


lazy_static! {
    pub static ref CHARMAP: BTreeMap<Keycode, (Option<char>, Option<char>)> = [
        (Keycode::KEY_Q, (Some('q'), Some('Q'))),
        (Keycode::KEY_W, (Some('w'), Some('W'))),
    ].into();
}

