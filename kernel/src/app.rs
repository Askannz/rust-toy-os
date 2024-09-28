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

use applib::geometry::Point2D;
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
            if !app.is_open || !deco.window_hover {
                None
            } else if deco.titlebar_hover {
                Some(Hover { app, kind: HoverKind::Titlebar })
            } else if deco.resize_hover {
                Some(Hover { app, kind: HoverKind::Resize })
            } else {
                Some(Hover { app, kind: HoverKind::Window })
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
    shadow_rect: Rect,
    resize_handle_rect: Rect,
    titlebar_font: &'static Font,
    titlebar_hover: bool,
    resize_hover: bool,
    window_hover: bool,
}

fn compute_decorations(app: &App, input_state: &InputState) -> AppDecorations {

    const DECO_PADDING: i64 = 5;
    const RESIZE_HANDLE_W: u32 = 10;
    const OFFSET_SHADOW: i64 = 10;

    let titlebar_font = &DEFAULT_FONT;

    let font_h = titlebar_font.char_h as u32;
    let titlebar_h = 3 * DECO_PADDING as u32 + font_h;

    let window_rect = Rect {
        x0: app.rect.x0 - DECO_PADDING,
        y0: app.rect.y0 - font_h as i64 - 2 * DECO_PADDING,
        w: app.rect.w + 2 * DECO_PADDING as u32,
        h: app.rect.h + titlebar_h,
    };

    let titlebar_rect = Rect {
        x0: window_rect.x0,
        y0: window_rect.y0,
        w: window_rect.w,
        h: titlebar_h,
    };

    let resize_handle_rect = Rect {
        x0: app.rect.x0 + (app.rect.w - RESIZE_HANDLE_W) as i64,
        y0: app.rect.y0 + (app.rect.h - RESIZE_HANDLE_W) as i64,
        w: RESIZE_HANDLE_W,
        h: RESIZE_HANDLE_W,
    };

    let shadow_rect = Rect {
        x0: window_rect.x0 + OFFSET_SHADOW,
        y0: window_rect.y0 + OFFSET_SHADOW,
        w: window_rect.w,
        h: window_rect.h,
    };

    let pointer = &input_state.pointer;
    let titlebar_hover = titlebar_rect.check_contains_point(pointer.x, pointer.y);
    let resize_hover = resize_handle_rect.check_contains_point(pointer.x, pointer.y);
    let window_hover = window_rect.check_contains_point(pointer.x, pointer.y);

    AppDecorations {
        content_rect: app.rect.clone(),
        window_rect,
        titlebar_rect,
        shadow_rect,
        resize_handle_rect,
        titlebar_font,
        titlebar_hover,
        resize_hover,
        window_hover,
    }
}


fn draw_app<F: FbViewMut>(fb: &mut F, app_name: &str, app_fb: &Framebuffer<BorrowedPixels>, deco: &AppDecorations) {

    const ALPHA_SHADOW: u8 = 100;

    const COLOR_IDLE: Color = Color::rgba(0x44, 0x44, 0x44, 0xff);
    const COLOR_HOVER: Color = Color::rgba(0x88, 0x88, 0x88, 0xff);
    const COLOR_SHADOW: Color = Color::rgba(0x0, 0x0, 0x0, ALPHA_SHADOW);
    const COLOR_TEXT: Color = Color::rgba(0xff, 0xff, 0xff, 0xff);
    const COLOR_RESIZE_HANDLE: Color = Color::rgba(0xff, 0xff, 0xff, 0x80);

    blend_rect(fb, &deco.shadow_rect, COLOR_SHADOW);
    draw_rect(fb, &deco.window_rect, COLOR_IDLE, false);

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

    fb.copy_from_fb(app_fb, deco.content_rect.origin(), false);

    blend_rect(fb, &deco.resize_handle_rect, COLOR_RESIZE_HANDLE);
}
