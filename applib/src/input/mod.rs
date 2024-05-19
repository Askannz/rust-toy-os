pub mod keymap;

use crate::Rect;
pub use keymap::{Keycode, CHARMAP};

pub const MAX_EVENTS: usize = 10;

#[derive(Debug, Clone)]
#[repr(C)]
pub struct InputState {
    pub pointer: PointerState,
    pub events: [Option<InputEvent>; MAX_EVENTS],
    next_event_index: usize,
}

impl InputState {

    pub fn new(w: u32, h: u32) -> Self {
        Self { 
            pointer: PointerState { 
                x: (w / 2).into(),
                y: (h / 2).into(),
                left_clicked: false,
                right_clicked: false,
            },
            events: [None; MAX_EVENTS],
            next_event_index: 0,
        }
    }

    pub fn clear_events(&mut self) {
        self.next_event_index = 0;
        self.events.fill(None);
    }

    pub fn add_event(&mut self, event: InputEvent) {
        if self.next_event_index < self.events.len() {
            self.events[self.next_event_index] = Some(event);
            self.next_event_index += 1;
        } else {
            log::warn!("Max input events {} reached, dropping event {:?}", MAX_EVENTS, event);
        }
    }

    pub fn change_origin(&self, rect: &Rect) -> Self {
    
        let mut new = self.clone();

        let PointerState { x, y, .. } = self.pointer;
        let &Rect { x0, y0, .. } = rect;

        new.pointer.x = x - x0;
        new.pointer.y = y - y0;

        new
    }
}

#[derive(Debug, Clone)]
#[repr(C)]

pub struct PointerState {
    pub x: i64,
    pub y: i64,
    pub left_clicked: bool,
    pub right_clicked: bool
}

#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub enum InputEvent {
    KeyPress { keycode: Keycode },
    KeyRelease { keycode: Keycode },
    Scroll { delta: i64 }
}
