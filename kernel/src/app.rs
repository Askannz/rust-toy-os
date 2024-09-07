use core::borrow::{Borrow, BorrowMut};

use alloc::rc::Rc;
use core::cell::RefCell;

use applib::drawing::primitives::{blend_rect, draw_rect};
use applib::drawing::text::{draw_str, DEFAULT_FONT};
use applib::input::{InputEvent, InputState};
use applib::uitk::{self, UiStore};
use applib::{decode_png, Color, FbViewMut, Framebuffer, OwnedPixels, Rect, SystemState};

use crate::system::System;
use crate::wasm::{WasmApp, WasmEngine};

#[derive(Clone)]
pub struct AppDescriptor {
    pub data: &'static [u8],
    pub launch_rect: Rect,
    pub name: &'static str,
    pub init_win_rect: Rect,
    pub icon: Option<&'static Framebuffer<OwnedPixels>>,
}

pub struct App {
    pub wasm_app: WasmApp,
    pub descriptor: AppDescriptor,
    pub is_open: bool,
    pub rect: Rect,
    pub grab_pos: Option<(i64, i64)>,
    pub time_used: f64,
}


impl AppDescriptor {

    pub fn instantiate(&self, system: Rc<RefCell<System>>, wasm_engine: &WasmEngine) -> App {

        App {
            descriptor: self.clone(),
            wasm_app: wasm_engine.instantiate_app(
                system.clone(),
                self.data,
                self.name,
                &self.init_win_rect,
            ),
            is_open: false,
            rect: self.init_win_rect.clone(),
            grab_pos: None,
            time_used: 0.0,
        }
    }

}