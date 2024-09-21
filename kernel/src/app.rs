use core::borrow::Borrow;

use crate::alloc::string::ToString;
use alloc::rc::Rc;
use applib::content::UuidProvider;
use core::cell::RefCell;

use applib::drawing::primitives::{blend_rect, draw_rect};
use applib::drawing::text::{draw_str, DEFAULT_FONT};
use applib::uitk::{self, UiContext};
use applib::{Color, FbViewMut, Framebuffer, OwnedPixels, Rect, SystemState};

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

pub struct App {
    pub wasm_app: WasmApp,
    pub descriptor: AppDescriptor,
    pub is_open: bool,
    pub rect: Rect,
    pub grab: GrabState,
    pub time_used: f64,
}


impl AppDescriptor {

    pub fn instantiate(&self, system: &mut System, system_state: &SystemState, wasm_engine: &WasmEngine) -> App {

        App {
            descriptor: self.clone(),
            wasm_app: wasm_engine.instantiate_app(
                system,
                system_state,
                self.data,
                self.name,
                &self.init_win_rect,
            ),
            is_open: false,
            rect: self.init_win_rect.clone(),
            grab: GrabState::None,
            time_used: 0.0,
        }
    }

}

impl App {

    pub fn step<F: FbViewMut, P: UuidProvider>(
        &mut self,
        uitk_context: &mut uitk::UiContext<F, P>,
        system: &mut System,
        system_state: &SystemState
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

        let app = self;

        let input_state = &system_state.input;
        let pointer_state = &input_state.pointer;

        let is_button_fired = uitk_context.button(&uitk::ButtonConfig {
            rect: app.descriptor.launch_rect.clone(),
            text: app.descriptor.name.to_string(),
            icon: app.descriptor.icon,
            ..Default::default()
        });

        if is_button_fired && !app.is_open {
            log::info!("{} is open", app.descriptor.name);
            app.is_open = true;
        }

        if app.is_open {

            let font_h = DEFAULT_FONT.char_h as u32;
            let titlebar_h = 3 * DECO_PADDING as u32 + font_h;
            let deco_rect = Rect {
                x0: app.rect.x0 - DECO_PADDING,
                y0: app.rect.y0 - font_h as i64 - 2 * DECO_PADDING,
                w: app.rect.w + 2 * DECO_PADDING as u32,
                h: app.rect.h + titlebar_h,
            };

            let resize_handle_rect = Rect {
                x0: app.rect.x0 + (app.rect.w - RESIZE_HANDLE_W) as i64,
                y0: app.rect.y0 + (app.rect.h - RESIZE_HANDLE_W) as i64,
                w: RESIZE_HANDLE_W,
                h: RESIZE_HANDLE_W,
            };

            if let GrabState::MoveGrab(dx, dy) = app.grab {
                if pointer_state.left_clicked {
                    app.rect.x0 = pointer_state.x - dx;
                    app.rect.y0 = pointer_state.y - dy;
                } else {
                    app.grab = GrabState::None;
                }
            } else if let GrabState::ResizeGrab = app.grab  {

                if pointer_state.left_clicked {

                    let [x1, y1, _, _] = app.rect.as_xyxy();
                    let x2 = pointer_state.x;
                    let y2 = pointer_state.y;
                    app.rect = Rect::from_xyxy([x1, y1, x2, y2]);

                } else {
                    app.grab = GrabState::None;
                }
            } else {
                let titlebar_rect = Rect {
                    x0: deco_rect.x0,
                    y0: deco_rect.y0,
                    w: deco_rect.w,
                    h: titlebar_h,
                };
                let titlebar_hover = titlebar_rect.check_contains_point(pointer_state.x, pointer_state.y);
                let resize_hover = resize_handle_rect.check_contains_point(pointer_state.x, pointer_state.y);

                // DEBUG
                if resize_hover {
                    log::debug!("RESIZE HOVER");
                }

                if titlebar_hover && pointer_state.left_click_trigger {
                    let dx = pointer_state.x - app.rect.x0;
                    let dy = pointer_state.y - app.rect.y0;

                    app.grab = GrabState::MoveGrab(dx, dy);

                } else if resize_hover && pointer_state.left_click_trigger {
                    app.grab = GrabState::ResizeGrab;
                } else if titlebar_hover && pointer_state.right_clicked {
                    app.is_open = false;
                }
            }

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

            let (x_txt, y_txt) = (app.rect.x0, app.rect.y0 - font_h as i64 - DECO_PADDING);
            draw_str(
                *fb,
                app.descriptor.name,
                x_txt,
                y_txt,
                &DEFAULT_FONT,
                COLOR_TEXT,
                None,
            );

            let t0 = system.clock.time();
            app.wasm_app.step(system, system_state, *fb, &app.rect);
            let t1 = system.clock.time();
            const SMOOTHING: f64 = 0.9;
            app.time_used = (1.0 - SMOOTHING) * (t1 - t0) + SMOOTHING * app.time_used;

            blend_rect(*fb, &resize_handle_rect, COLOR_RESIZE_HANDLE);
        }
    }

}