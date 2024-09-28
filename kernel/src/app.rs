use core::borrow::Borrow;

use crate::alloc::string::ToString;
use crate::app;
use alloc::borrow::ToOwned;
use alloc::collections::btree_map::BTreeMap;
use alloc::string::String;
use alloc::rc::Rc;
use uefi::proto::console::pointer;
use core::cell::RefCell;
use alloc::vec::Vec;

use applib::geometry::Point2D;
use applib::drawing::primitives::{blend_rect, draw_rect};
use applib::drawing::text::{draw_str, DEFAULT_FONT, HACK_15};
use applib::uitk::{self, UiContext};
use applib::{Color, FbViewMut, Framebuffer, OwnedPixels, Rect, input::InputState};
use crate::shell::{pie_menu, PieMenuEntry};

use crate::system::System;
use crate::wasm::{WasmApp, WasmEngine};

#[derive(Clone)]
pub struct AppDescriptor {
    pub data: &'static [u8],
    pub launch_rect: Rect,
    pub name: &'static str,
    pub init_win_rect: Rect,
    pub icon: &'static Framebuffer<OwnedPixels>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AppsInteractionState {
    Idle,
    TitlebarHold { app_name: &'static str, anchor: Point2D<i64> },
    ResizeHold { app_name: &'static str },
    PieMenu { anchor: Point2D<i64> },
}

pub struct App {
    pub wasm_app: WasmApp,
    pub descriptor: AppDescriptor,
    pub is_open: bool,
    pub rect: Rect,
    pub interaction_state: AppsInteractionState,
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
            interaction_state: AppsInteractionState::Idle,
            time_used: 0.0,
        }
    }

}

pub fn run_apps<F: FbViewMut>(
    uitk_context: &mut uitk::UiContext<F>,
    system: &mut System,
    apps: &mut BTreeMap<&str, App>,
    input_state: &InputState,
    interaction_state: &mut AppsInteractionState,
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

    struct HoverState {
        titlebar: bool,
        window: bool,
        resize: bool,
    }
    
    let pointer_state = &input_state.pointer;

    if let AppsInteractionState::TitlebarHold { app_name, anchor}  = interaction_state {

        if let Some(app) = apps.get_mut(app_name).filter(|app| app.is_open) {
            app.rect.x0 = pointer_state.x - anchor.x;
            app.rect.y0 = pointer_state.y - anchor.y;
        }

    } else if let AppsInteractionState::ResizeHold { app_name } = interaction_state {

        if let Some(app) = apps.get_mut(app_name).filter(|app| app.is_open) {
            let [x1, y1, _, _] = app.rect.as_xyxy();
            let x2 = pointer_state.x;
            let y2 = pointer_state.y;
            app.rect = Rect::from_xyxy([x1, y1, x2, y2]);
        }
    }

    let UiContext { fb, .. } = uitk_context;

    let hover_states: BTreeMap<&'static str, HoverState> = apps.values_mut().map(|app| {

        let app_name = app.descriptor.name;

        if !app.is_open { return (app_name, HoverState { titlebar: false, window: false, resize: false }) }

        let font_h = DEFAULT_FONT.char_h as u32;
        let titlebar_h = 3 * DECO_PADDING as u32 + font_h;

        let deco_rect = Rect {
            x0: app.rect.x0 - DECO_PADDING,
            y0: app.rect.y0 - font_h as i64 - 2 * DECO_PADDING,
            w: app.rect.w + 2 * DECO_PADDING as u32,
            h: app.rect.h + titlebar_h,
        };

        let titlebar_rect = Rect {
            x0: deco_rect.x0,
            y0: deco_rect.y0,
            w: deco_rect.w,
            h: titlebar_h,
        };

        let resize_handle_rect = Rect {
            x0: app.rect.x0 + (app.rect.w - RESIZE_HANDLE_W) as i64,
            y0: app.rect.y0 + (app.rect.h - RESIZE_HANDLE_W) as i64,
            w: RESIZE_HANDLE_W,
            h: RESIZE_HANDLE_W,
        };

        let shadow_rect = Rect {
            x0: deco_rect.x0 + OFFSET_SHADOW,
            y0: deco_rect.y0 + OFFSET_SHADOW,
            w: deco_rect.w,
            h: deco_rect.h,
        };

        let hover = HoverState {
            titlebar: titlebar_rect.check_contains_point(pointer_state.x, pointer_state.y),
            window: deco_rect.check_contains_point(pointer_state.x, pointer_state.y),
            resize: resize_handle_rect.check_contains_point(pointer_state.x, pointer_state.y)
        };


        let color_app = match hover.window && interaction_state == &AppsInteractionState::Idle {
            true => COLOR_HOVER,
            false => COLOR_IDLE,
        };

        blend_rect(*fb, &shadow_rect, COLOR_SHADOW);
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
        let wasm_fb = app.wasm_app.step(system, input_state, &app.rect);
        let t1 = system.clock.time();
        const SMOOTHING: f64 = 0.9;
        app.time_used = (1.0 - SMOOTHING) * (t1 - t0) + SMOOTHING * app.time_used;

        fb.copy_from_fb(&wasm_fb, app.rect.origin(), false);

        blend_rect(*fb, &resize_handle_rect, COLOR_RESIZE_HANDLE);

        (app_name, hover)
    })
    .collect();

    if let AppsInteractionState::PieMenu { anchor } = *interaction_state {

        let entries: Vec<PieMenuEntry> = apps.values().map(|app| {

            PieMenuEntry { 
                icon: app.descriptor.icon,
                bg_color: Color::rgba(0x44, 0x44, 0x44, 0xff),
                text: app.descriptor.name.to_owned(),
                text_color: Color::WHITE,
                font: &HACK_15,
            }

        }).collect();

        let selected = pie_menu(uitk_context, &entries, anchor);

        if let Some(entry) = selected {
            let app_name = &entry.text;
            apps.get_mut(app_name.as_str()).unwrap().is_open = true;
        }

    }


    //
    // Update interaction state

    let hovered_titlebar = hover_states.iter().find_map(|(app_name, hover)| match hover.titlebar {
        false => None,
        true => apps.get(app_name),
    });

    let hovered_resize = hover_states.iter().find_map(|(app_name, hover)| match hover.resize {
        false => None,
        true => apps.get(app_name),
    });


    if *interaction_state == AppsInteractionState::Idle {

        if pointer_state.left_clicked {

            if let Some(app) = hovered_titlebar {
                let dx = pointer_state.x - app.rect.x0;
                let dy = pointer_state.y - app.rect.y0;
                *interaction_state = AppsInteractionState::TitlebarHold { 
                    app_name: app.descriptor.name,
                    anchor: Point2D { x: dx, y: dy }
                };
            } else if let Some(app) = hovered_resize {
                *interaction_state = AppsInteractionState::ResizeHold { app_name: app.descriptor.name };
            }

        } else if pointer_state.right_clicked {
            let anchor = Point2D { x: pointer_state.x, y: pointer_state.y };
            *interaction_state = AppsInteractionState::PieMenu { anchor };
        }

    } else {
        if !pointer_state.left_clicked && !pointer_state.right_clicked {
            *interaction_state = AppsInteractionState::Idle;
        }
    }
}
