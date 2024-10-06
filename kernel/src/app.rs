use core::borrow::Borrow;

use crate::alloc::string::ToString;
use crate::app;
use alloc::borrow::ToOwned;
use alloc::collections::btree_map::BTreeMap;
use alloc::string::String;
use alloc::rc::Rc;
use applib::BorrowedPixels;
use uefi::proto::console::pointer;
use core::cell::RefCell;
use alloc::vec::Vec;
use lazy_static::lazy_static;

use applib::geometry::{Point2D, Vec2D};
use applib::drawing::primitives::{blend_rect, draw_rect};
use applib::drawing::text::{draw_str, Font, DEFAULT_FONT, HACK_15};
use applib::uitk::{self, UiContext};
use applib::{Color, FbViewMut, Framebuffer, OwnedPixels, Rect, input::InputState};
use crate::shell::{pie_menu, PieMenuEntry};

use crate::system::System;
use crate::wasm::{WasmApp, WasmEngine};
use crate::resources;


#[derive(Clone)]
pub struct AppDescriptor {
    pub data: &'static [u8],
    pub name: &'static str,
    pub init_win_rect: Rect,
    pub icon: &'static Framebuffer<OwnedPixels>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AppsInteractionState {
    Idle,
    TitlebarHold { app_name: &'static str, anchor: Point2D<i64> },
    ResizeHold { app_name: &'static str },
    PieDesktopMenu { anchor: Point2D<i64> },
    PieAppMenu { app_name: &'static str, anchor: Point2D<i64> },
}

pub struct App {
    pub wasm_app: WasmApp,
    pub descriptor: AppDescriptor,
    pub is_open: bool,
    pub rect: Rect,
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

    const PIE_DEFAULT_COLOR: Color = Color::rgb(0x44, 0x44, 0x44);

    let pointer = &input_state.pointer;

    //
    // Hover

    #[derive(Debug, Clone, Copy, PartialEq)]
    enum HoverKind {
        Titlebar,
        Resize,
        Window,
    }

    struct Hover<'a> {
        app: &'a App,
        kind: HoverKind,
    }

    let hover_state = apps.values()
        .map(|app| {
            let deco = compute_decorations(app, input_state);
            (app, deco)
        })
        .find_map(|(app, deco)| {
            if !app.is_open {
                None
            } else if deco.titlebar_hover {
                Some(Hover { app, kind: HoverKind::Titlebar })
            } else if deco.resize_hover {
                Some(Hover { app, kind: HoverKind::Resize })
            } else if deco.window_hover {
                Some(Hover { app, kind: HoverKind::Window })
            } else {
                None
            }
        });


    //
    // Interaction state update

    *interaction_state = match *interaction_state {

        AppsInteractionState::Idle if pointer.left_click_trigger =>  match hover_state {

            Some(Hover { app, kind }) if kind == HoverKind::Titlebar => {
                let dx = pointer.x - app.rect.x0;
                let dy = pointer.y - app.rect.y0;
                AppsInteractionState::TitlebarHold { 
                    app_name: app.descriptor.name,
                    anchor: Point2D { x: dx, y: dy }
                }
            },

            Some(Hover { app, kind }) if kind == HoverKind::Resize => {
                AppsInteractionState::ResizeHold { app_name: app.descriptor.name }
            },

            _ => *interaction_state,
        },

        AppsInteractionState::Idle if pointer.right_click_trigger =>  match hover_state {

            Some(Hover { app, .. }) => {
                let anchor = Point2D { x: pointer.x, y: pointer.y };
                AppsInteractionState::PieAppMenu { app_name: app.descriptor.name , anchor }
            },

            _ => {
                let anchor = Point2D { x: pointer.x, y: pointer.y };
                AppsInteractionState::PieDesktopMenu { anchor }
            }
        },

        AppsInteractionState::TitlebarHold { .. } if !pointer.left_clicked => AppsInteractionState::Idle,

        AppsInteractionState::ResizeHold { .. } if !pointer.left_clicked => AppsInteractionState::Idle,

        AppsInteractionState::PieAppMenu { .. } if pointer.right_click_trigger => AppsInteractionState::Idle,

        AppsInteractionState::PieDesktopMenu { .. } if pointer.right_click_trigger => AppsInteractionState::Idle,

        _ => *interaction_state,
    };

    //
    // Update window rects

    if let AppsInteractionState::TitlebarHold { app_name, anchor}  = interaction_state {

        if let Some(app) = apps.get_mut(app_name).filter(|app| app.is_open) {
            app.rect.x0 = pointer.x - anchor.x;
            app.rect.y0 = pointer.y - anchor.y;
        }

    } else if let AppsInteractionState::ResizeHold { app_name } = interaction_state {

        if let Some(app) = apps.get_mut(app_name).filter(|app| app.is_open) {
            let [x1, y1, _, _] = app.rect.as_xyxy();
            let x2 = pointer.x;
            let y2 = pointer.y;
            app.rect = Rect::from_xyxy([x1, y1, x2, y2]);
        }
    }
    

    //
    // Step and draw apps

    let UiContext { fb, .. } = uitk_context;

    for (app_name, app) in apps.iter_mut() {

        if !app.is_open { continue; }

        let deco = compute_decorations(&app, input_state);
        let app_fb = app.wasm_app.step(system, input_state, &app.rect);

        draw_app(*fb, app_name, &app_fb, &deco);
    }

    
    //
    // Pie menus

    if let AppsInteractionState::PieDesktopMenu { anchor } = *interaction_state {

        let entries: Vec<PieMenuEntry> = apps.values().map(|app| {

            PieMenuEntry::Button { 
                icon: app.descriptor.icon,
                color: PIE_DEFAULT_COLOR,
                text: app.descriptor.name.to_owned(),
                text_color: Color::WHITE,
                font: &HACK_15,
                weight: 1.0,
            }

        }).collect();

        let selected = pie_menu(uitk_context, &entries, anchor);

        if let Some(app_name) = selected {

            let app = apps.get_mut(app_name).unwrap();

            app.is_open = true;
            app.rect = Rect::from_center(
                pointer.x,
                pointer.y,
                app.rect.w,
                app.rect.h
            );

            *interaction_state = AppsInteractionState::Idle;
        }

    } else if let AppsInteractionState::PieAppMenu { app_name, anchor } = *interaction_state {

        if let Some(app) = apps.get_mut(app_name) {

            let entries = [
                PieMenuEntry::Button { 
                    icon: &resources::CLOSE_ICON,
                    color: Color::rgb(180, 0, 0),
                    text: "Close".to_owned(),
                    text_color: Color::WHITE,
                    font: &HACK_15,
                    weight: 1.0,
                },
                PieMenuEntry::Spacer { color: PIE_DEFAULT_COLOR, weight: 1.0 },
                PieMenuEntry::Button { 
                    icon: &resources::RELOAD_ICON,
                    color: Color::rgb(180, 180, 0),
                    text: "Reload".to_owned(),
                    text_color: Color::WHITE,
                    font: &HACK_15,
                    weight: 1.0,
                },
                PieMenuEntry::Spacer { color: PIE_DEFAULT_COLOR, weight: 3.0 },
            ];

            let selected = pie_menu(uitk_context, &entries, anchor);

            if selected == Some("Close") {
                app.is_open = false;
                *interaction_state = AppsInteractionState::Idle;
            }
        }
    }
}


struct AppDecorations {
    content_rect: Rect,
    window_rect: Rect,
    titlebar_rect: Rect,
    resize_zone_rect: Rect,
    border_rects: [Rect; 3],
    handle_rects: [Rect; 2],
    titlebar_font: &'static Font,
    titlebar_hover: bool,
    resize_hover: bool,
    window_hover: bool,
}

fn compute_decorations(app: &App, input_state: &InputState) -> AppDecorations {

    const TITLEBAR_HEIGHT: u32 = 32;
    const BORDER_THICKNESS: u32 = 8;
    const RESIZE_HANDLE_LEN: u32 = 32;
    const RESIZE_HANDLE_GAP: u32 = 2;
    const RESIZE_ZONE_LEN: u32 = 32;
    const RESIZE_HANDLE_OFFSET: u32 = 4;
    let FONT: &'static Font = &DEFAULT_FONT;

    let window_rect = Rect {
        x0: app.rect.x0 - BORDER_THICKNESS as i64,
        y0: app.rect.y0 - TITLEBAR_HEIGHT as i64,
        w: app.rect.w + 2 * BORDER_THICKNESS,
        h: app.rect.h + TITLEBAR_HEIGHT,
    };

    let titlebar_rect = Rect {
        x0: window_rect.x0,
        y0: window_rect.y0,
        w: window_rect.w,
        h: TITLEBAR_HEIGHT,
    };

    let left_border_rect = Rect {
        x0: window_rect.x0,
        y0: window_rect.y0 + TITLEBAR_HEIGHT as i64,
        w: BORDER_THICKNESS,
        h: app.rect.h,
    };

    let bottom_border_rect = Rect {
        x0: window_rect.x0,
        y0: app.rect.y0 + app.rect.h as i64,
        w:  window_rect.w - RESIZE_HANDLE_GAP - RESIZE_HANDLE_LEN,
        h: BORDER_THICKNESS,
    };

    let right_border_rect = Rect {
        x0: window_rect.x0 + BORDER_THICKNESS as i64 + app.rect.w as i64,
        y0: window_rect.y0 + TITLEBAR_HEIGHT as i64,
        w: BORDER_THICKNESS,
        h: app.rect.h - RESIZE_HANDLE_GAP - RESIZE_HANDLE_LEN + BORDER_THICKNESS,
    };

    let resize_zone_rect = Rect::from_center(
        app.rect.x0 + app.rect.w as i64,
        app.rect.y0 + app.rect.h as i64,
        RESIZE_ZONE_LEN, 
        RESIZE_ZONE_LEN,
    );

    let mut handle_rect_1 = Rect {
        x0: bottom_border_rect.x0 + bottom_border_rect.w as i64 + RESIZE_HANDLE_GAP as i64,
        y0: bottom_border_rect.y0,
        w: RESIZE_HANDLE_LEN,
        h: BORDER_THICKNESS,
    };

    let mut handle_rect_2 = Rect {
        x0: app.rect.x0 + app.rect.w as i64,
        y0: right_border_rect.y0 + right_border_rect.h as i64 + RESIZE_HANDLE_GAP as i64,
        w: BORDER_THICKNESS,
        h: RESIZE_HANDLE_LEN - BORDER_THICKNESS,
    };

    let pointer = &input_state.pointer;
    let titlebar_hover = titlebar_rect.check_contains_point(pointer.x, pointer.y);
    let resize_hover = resize_zone_rect.check_contains_point(pointer.x, pointer.y);
    let window_hover = window_rect.check_contains_point(pointer.x, pointer.y);

    if resize_hover {
        let offet_vec = Vec2D { x: 1, y: 1 } * RESIZE_HANDLE_OFFSET as i64;
        handle_rect_1 = handle_rect_1 + offet_vec;
        handle_rect_2 = handle_rect_2 + offet_vec;
    }

    AppDecorations {
        content_rect: app.rect.clone(),
        window_rect,
        titlebar_rect,
        titlebar_font: FONT,
        titlebar_hover,
        resize_hover,
        window_hover,
        border_rects: [
            left_border_rect,
            right_border_rect,
            bottom_border_rect,
        ],
        handle_rects: [handle_rect_1, handle_rect_2],
        resize_zone_rect,
    }
}


fn draw_app<F: FbViewMut>(fb: &mut F, app_name: &str, app_fb: &Framebuffer<BorrowedPixels>, deco: &AppDecorations) {

    const COLOR_IDLE: Color = Color::rgba(0x44, 0x44, 0x44, 0xff);
    const COLOR_HANDLE: Color = Color::rgba(122, 0, 255, 0xff);
    const COLOR_TEXT: Color = Color::rgba(0xff, 0xff, 0xff, 0xff);
    
    draw_rect(fb, &deco.titlebar_rect, COLOR_IDLE, false);
    for rect in deco.border_rects.iter() {
        draw_rect(fb, rect, COLOR_IDLE, false);
    }

    let text_h = deco.titlebar_font.char_h as u32;
    let text_w = (deco.titlebar_font.char_w * app_name.len()) as u32;

    let padding = (deco.titlebar_rect.h - text_h) / 2;
    let text_rect = Rect { 
        x0: deco.titlebar_rect.x0 + padding as i64,
        y0: 0,
        w: text_w,
        h: text_h
    }.align_to_rect_vert(&deco.titlebar_rect);

    draw_str(
        fb,
        app_name,
        text_rect.x0,
        text_rect.y0,
        &deco.titlebar_font,
        COLOR_TEXT,
        None,
    );

    fb.copy_from_fb_2(app_fb, deco.content_rect.origin(), false);

    for rect in deco.handle_rects.iter() {
        draw_rect(fb, rect, COLOR_HANDLE, false);
    }
}
