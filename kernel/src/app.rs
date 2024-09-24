use core::borrow::Borrow;

use crate::alloc::string::ToString;
use alloc::rc::Rc;
use core::cell::RefCell;

use applib::geometry::Point2D;
use applib::drawing::primitives::{blend_rect, draw_rect};
use applib::drawing::text::{draw_str, DEFAULT_FONT};
use applib::uitk::{self, UiContext};
use applib::{Color, FbViewMut, Framebuffer, OwnedPixels, Rect, input::InputState};

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

enum GrabState {
    None,
    MoveGrab(i64, i64),
    ResizeGrab,
}

#[derive(Debug, Clone, Copy)]
enum InteractionState {
    Idle,
    TitlebarHold(Point2D<i64>),
    ResizeHold,
}

pub struct App {
    pub wasm_app: WasmApp,
    pub descriptor: AppDescriptor,
    pub is_open: bool,
    pub rect: Rect,
    pub interaction_state: InteractionState,
    pub time_used: f64,
}


impl AppDescriptor {

    pub fn instantiate(&self, system: &mut System, input_state: &InputState, wasm_engine: &WasmEngine) -> App {

        App {
            descriptor: self.clone(),
            wasm_app: wasm_engine.instantiate_app(
                system,
                input_state,
                self.data,
                self.name,
                &self.init_win_rect,
            ),
            is_open: false,
            rect: self.init_win_rect.clone(),
            interaction_state: InteractionState::Idle,
            time_used: 0.0,
        }
    }

}

impl App {

    pub fn step<F: FbViewMut>(
        &mut self,
        uitk_context: &mut uitk::UiContext<F>,
        system: &mut System,
        input_state: &InputState
    ) {

        const ALPHA_SHADOW: u8 = 100;

        const COLOR_IDLE: Color = Color::rgba(0x44, 0x44, 0x44, 0xff);
        const COLOR_HOVER: Color = Color::rgba(0x88, 0x88, 0x88, 0xff);
        const COLOR_SHADOW: Color = Color::rgba(0x0, 0x0, 0x0, ALPHA_SHADOW);
        const COLOR_TEXT: Color = Color::rgba(0xff, 0xff, 0xff, 0xff);
        const COLOR_RESIZE_HANDLE: Color = Color::rgba(0xff, 0xff, 0xff, 0x80);
    
        const OFFSET_SHADOW: i64 = 10;
        const DECO_PADDING: i64 = 5;
        const RESIZE_HANDLE_W: u32 = 10;

        let pointer_state = &input_state.pointer;

        let is_button_fired = uitk_context.button(&uitk::ButtonConfig {
            rect: self.descriptor.launch_rect.clone(),
            text: self.descriptor.name.to_string(),
            icon: self.descriptor.icon,
            ..Default::default()
        });

        if is_button_fired && !self.is_open {
            log::info!("{} is open", self.descriptor.name);
            self.is_open = true;
        }

        if !self.is_open {
            return;
        }

        if let InteractionState::TitlebarHold(anchor) = self.interaction_state {
            self.rect.x0 = pointer_state.x - anchor.x;
            self.rect.y0 = pointer_state.y - anchor.y;
        } else if let InteractionState::ResizeHold = self.interaction_state {
            let [x1, y1, _, _] = self.rect.as_xyxy();
            let x2 = pointer_state.x;
            let y2 = pointer_state.y;
            self.rect = Rect::from_xyxy([x1, y1, x2, y2]);
        }

        let font_h = DEFAULT_FONT.char_h as u32;
        let titlebar_h = 3 * DECO_PADDING as u32 + font_h;

        let deco_rect = Rect {
            x0: self.rect.x0 - DECO_PADDING,
            y0: self.rect.y0 - font_h as i64 - 2 * DECO_PADDING,
            w: self.rect.w + 2 * DECO_PADDING as u32,
            h: self.rect.h + titlebar_h,
        };

        let titlebar_rect = Rect {
            x0: deco_rect.x0,
            y0: deco_rect.y0,
            w: deco_rect.w,
            h: titlebar_h,
        };

        let resize_handle_rect = Rect {
            x0: self.rect.x0 + (self.rect.w - RESIZE_HANDLE_W) as i64,
            y0: self.rect.y0 + (self.rect.h - RESIZE_HANDLE_W) as i64,
            w: RESIZE_HANDLE_W,
            h: RESIZE_HANDLE_W,
        };

        let titlebar_hover = titlebar_rect.check_contains_point(pointer_state.x, pointer_state.y);
        let resize_hover = resize_handle_rect.check_contains_point(pointer_state.x, pointer_state.y);

        self.interaction_state = match pointer_state.left_clicked {
            false => InteractionState::Idle,
            true if titlebar_hover => {
                let dx = pointer_state.x - self.rect.x0;
                let dy = pointer_state.y - self.rect.y0;
                InteractionState::TitlebarHold(Point2D { x: dx, y: dy })
            },
            true if resize_hover => InteractionState::ResizeHold,
            _ => InteractionState::Idle
        };

        let UiContext { fb, .. } = uitk_context;

        let shadow_rect = Rect {
            x0: deco_rect.x0 + OFFSET_SHADOW,
            y0: deco_rect.y0 + OFFSET_SHADOW,
            w: deco_rect.w,
            h: deco_rect.h,
        };

        blend_rect(*fb, &shadow_rect, COLOR_SHADOW);

        let instance_hover = deco_rect.check_contains_point(pointer_state.x, pointer_state.y);
        let color_app = if instance_hover {
            COLOR_HOVER
        } else {
            COLOR_IDLE
        };
        draw_rect(*fb, &deco_rect, color_app);

        let (x_txt, y_txt) = (self.rect.x0, self.rect.y0 - font_h as i64 - DECO_PADDING);
        draw_str(
            *fb,
            self.descriptor.name,
            x_txt,
            y_txt,
            &DEFAULT_FONT,
            COLOR_TEXT,
            None,
        );

        let t0 = system.clock.time();
        self.wasm_app.step(system, input_state, *fb, &self.rect);
        let t1 = system.clock.time();
        const SMOOTHING: f64 = 0.9;
        self.time_used = (1.0 - SMOOTHING) * (t1 - t0) + SMOOTHING * self.time_used;

        blend_rect(*fb, &resize_handle_rect, COLOR_RESIZE_HANDLE);
    }

}