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

    BTN_MOUSE_LEFT = 272,
    BTN_MOUSE_RIGHT = 273,
    BTN_GEAR_DOWN = 336,
    BTN_GEAR_UP = 337,

    KEY_1 = 2,
    KEY_2 = 3,
    KEY_3 = 4,
    KEY_4 = 5,
    KEY_5 = 6,
    KEY_6 = 7,
    KEY_7 = 8,
    KEY_8 = 9,
    KEY_9 = 10,
    KEY_0 = 11,

    KEY_MINUS = 12,
    KEY_EQUAL = 13,
    KEY_LEFTBRACE = 26,
    KEY_RIGHTBRACE = 27,
    KEY_BACKSLASH = 43,
    KEY_SEMICOLON = 39,
    KEY_APOSTROPHE = 40,
    KEY_COMMA = 51,
    KEY_DOT = 52,
    KEY_SLASH = 53,

    KEY_Q = 16,
    KEY_W = 17,
    KEY_E = 18,
    KEY_R = 19,
    KEY_T = 20,
    KEY_Y = 21,
    KEY_U = 22,
    KEY_I = 23,
    KEY_O = 24,
    KEY_P = 25,
    KEY_A = 30,
    KEY_S = 31,
    KEY_D = 32,
    KEY_F = 33,
    KEY_G = 34,
    KEY_H = 35,
    KEY_J = 36,
    KEY_K = 37,
    KEY_L = 38,
    KEY_Z = 44,
    KEY_X = 45,
    KEY_C = 46,
    KEY_V = 47,
    KEY_B = 48,
    KEY_N = 49,
    KEY_M = 50,

    KEY_BACKSPACE = 14,
    KEY_ENTER = 28,
    KEY_LEFTSHIFT = 42,
    KEY_RIGHTSHIFT = 54,
    KEY_SPACE = 57,
}


lazy_static! {
    pub static ref CHARMAP: BTreeMap<Keycode, (Option<char>, Option<char>)> = [

    (Keycode::KEY_1, (Some('1'), Some('!'))),
    (Keycode::KEY_2, (Some('2'), Some('@'))),
    (Keycode::KEY_3, (Some('3'), Some('#'))),
    (Keycode::KEY_4, (Some('4'), Some('$'))),
    (Keycode::KEY_5, (Some('5'), Some('%'))),
    (Keycode::KEY_6, (Some('6'), Some('^'))),
    (Keycode::KEY_7, (Some('7'), Some('&'))),
    (Keycode::KEY_8, (Some('8'), Some('*'))),
    (Keycode::KEY_9, (Some('9'), Some('('))),
    (Keycode::KEY_0, (Some('0'), Some(')'))),

    (Keycode::KEY_MINUS, (Some('-'), Some('_'))),
    (Keycode::KEY_EQUAL, (Some('='), Some('+'))),
    (Keycode::KEY_LEFTBRACE, (Some('['), Some('{'))),
    (Keycode::KEY_RIGHTBRACE, (Some(']'), Some('}'))),
    (Keycode::KEY_BACKSLASH, (Some('\\'), Some('|'))),
    (Keycode::KEY_SEMICOLON, (Some(';'), Some(':'))),
    (Keycode::KEY_APOSTROPHE, (Some('\''), Some('"'))),
    (Keycode::KEY_COMMA, (Some(','), Some('<'))),
    (Keycode::KEY_DOT, (Some('.'), Some('>'))),
    (Keycode::KEY_SLASH, (Some('/'), Some('?'))),

    (Keycode::KEY_Q, (Some('q'), Some('Q'))),
    (Keycode::KEY_W, (Some('w'), Some('W'))),
    (Keycode::KEY_E, (Some('e'), Some('E'))),
    (Keycode::KEY_R, (Some('r'), Some('R'))),
    (Keycode::KEY_T, (Some('t'), Some('T'))),
    (Keycode::KEY_Y, (Some('y'), Some('Y'))),
    (Keycode::KEY_U, (Some('u'), Some('U'))),
    (Keycode::KEY_I, (Some('i'), Some('I'))),
    (Keycode::KEY_O, (Some('o'), Some('O'))),
    (Keycode::KEY_P, (Some('p'), Some('P'))),
    (Keycode::KEY_A, (Some('a'), Some('A'))),
    (Keycode::KEY_S, (Some('s'), Some('S'))),
    (Keycode::KEY_D, (Some('d'), Some('D'))),
    (Keycode::KEY_F, (Some('f'), Some('F'))),
    (Keycode::KEY_G, (Some('g'), Some('G'))),
    (Keycode::KEY_H, (Some('h'), Some('H'))),
    (Keycode::KEY_J, (Some('j'), Some('J'))),
    (Keycode::KEY_K, (Some('k'), Some('K'))),
    (Keycode::KEY_L, (Some('l'), Some('L'))),
    (Keycode::KEY_Z, (Some('z'), Some('Z'))),
    (Keycode::KEY_X, (Some('x'), Some('X'))),
    (Keycode::KEY_C, (Some('c'), Some('C'))),
    (Keycode::KEY_V, (Some('v'), Some('V'))),
    (Keycode::KEY_B, (Some('b'), Some('B'))),
    (Keycode::KEY_N, (Some('n'), Some('N'))),
    (Keycode::KEY_M, (Some('m'), Some('M'))),

    (Keycode::KEY_SPACE, (Some(' '), Some(' '))),

    ].into();
}

