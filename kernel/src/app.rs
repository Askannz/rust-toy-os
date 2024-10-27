use alloc::borrow::ToOwned;
use alloc::collections::btree_map::BTreeMap;
use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;
use applib::{BorrowedPixels, StyleSheet};

use crate::shell::{pie_menu, PieDrawCalls, PieMenuEntry};
use applib::drawing::primitives::draw_rect;
use applib::drawing::text::{draw_str, Font, DEFAULT_FONT, HACK_15};
use applib::geometry::{Point2D, Vec2D};
use applib::uitk::{self, UiContext};
use applib::{input::InputState, Color, FbViewMut, Framebuffer, OwnedPixels, Rect};

use crate::{app, resources};
use crate::system::System;
use crate::wasm::{WasmApp, WasmEngine};

#[derive(Clone)]
pub struct AppDescriptor {
    pub data: &'static [u8],
    pub name: &'static str,
    pub init_win_rect: Rect,
    pub icon: &'static Framebuffer<OwnedPixels>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum HoverKind {
    Titlebar,
    Resize,
    Window,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AppsInteractionState {
    Idle,
    AppHover {
        app_name: &'static str,
        hover_kind: HoverKind,
    },
    TitlebarHold {
        app_name: &'static str,
        anchor: Point2D<i64>,
    },
    ResizeHold {
        app_name: &'static str,
    },
    PieDesktopMenu {
        anchor: Point2D<i64>,
    },
    PieAppMenu {
        app_name: &'static str,
        anchor: Point2D<i64>,
    },
    // first_frame is to prevent the pie menus from capturing the mouse click
    // triggers immediately and closing after one frame
}

pub struct App {
    pub wasm_app: WasmApp,
    pub descriptor: AppDescriptor,
    pub is_open: bool,
    pub rect: Rect,
    pub z_order: usize,
    pub time_used: f64,
}

impl AppDescriptor {
    pub fn instantiate(
        &self,
        system: &mut System,
        input_state: &InputState,
        wasm_engine: &WasmEngine,
    ) -> App {
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
            z_order: 0,
            time_used: 0.0,
        }
    }
}

pub fn run_apps<F: FbViewMut>(
    uitk_context: &mut uitk::UiContext<F>,
    system: &mut System,
    apps: &mut BTreeMap<&'static str, App>,
    input_state: &InputState,
    interaction_state: &mut AppsInteractionState,
) {
    const MIN_APP_SIZE: u32 = 200;

    let stylesheet = system.stylesheet.clone();
    let pointer = &input_state.pointer;
    let mut pie_draw_calls: Option<PieDrawCalls> = None;

    //
    // Hover

    let hover_state = get_z_sorted_apps(apps).iter().rev()
        .map(|(app_name, app)| {
            let deco = compute_decorations(app, input_state);
            (app_name, app, deco)
        })
        .find_map(|(app_name, app, deco)| {
            if !app.is_open { None }
            else if deco.titlebar_hover { Some((*app_name, HoverKind::Titlebar)) }
            else if deco.resize_hover { Some((*app_name, HoverKind::Resize)) }
            else if deco.window_hover { Some((*app_name, HoverKind::Window)) }
            else { None }
        });

    //
    // Interaction state update

    let is = interaction_state;

    match *is {

        AppsInteractionState::Idle => match hover_state {
            None if pointer.right_click_trigger => {
                let anchor = Point2D { x: pointer.x, y: pointer.y };
                *is = AppsInteractionState::PieDesktopMenu { anchor };
            },
            None => (),
            Some((app_name, hover_kind)) => *is = AppsInteractionState::AppHover { app_name, hover_kind }
        },

        AppsInteractionState::AppHover { app_name, .. } if pointer.right_click_trigger => {
            let anchor = Point2D { x: pointer.x, y: pointer.y };
            let z_max = get_z_max(&apps);
            apps.get_mut(app_name).unwrap().z_order = z_max + 1;
            *is = AppsInteractionState::PieAppMenu { app_name, anchor };
        },

        AppsInteractionState::AppHover { app_name, hover_kind } if pointer.left_click_trigger => {

            let z_max = get_z_max(&apps);
            apps.get_mut(app_name).unwrap().z_order = z_max + 1;

            match hover_kind {
                HoverKind::Titlebar => {
                    let app = apps.get_mut(app_name).unwrap();
                    let dx = pointer.x - app.rect.x0;
                    let dy = pointer.y - app.rect.y0;
                    let anchor = Point2D { x: dx, y: dy };
                    *is =  AppsInteractionState::TitlebarHold { app_name, anchor };
                },

                HoverKind::Resize => *is = AppsInteractionState::ResizeHold { app_name },

                HoverKind::Window => (),
            }
        },

        AppsInteractionState::AppHover { .. } => match hover_state {
            None => *is = AppsInteractionState::Idle,
            Some((app_name, hover_kind)) => *is = AppsInteractionState::AppHover { app_name, hover_kind }
        },

        AppsInteractionState::TitlebarHold { .. } if !pointer.left_clicked => {
            *is = AppsInteractionState::Idle;
        },

        AppsInteractionState::TitlebarHold { app_name, anchor } => {
            let app = apps.get_mut(app_name).unwrap();
            app.rect.x0 = pointer.x - anchor.x;
            app.rect.y0 = pointer.y - anchor.y;
        },

        AppsInteractionState::ResizeHold { .. } if !pointer.left_clicked => {
            *is = AppsInteractionState::Idle;
        },

        AppsInteractionState::ResizeHold { app_name } => {
            let app = apps.get_mut(app_name).unwrap();
            let [x1, y1, _, _] = app.rect.as_xyxy();
            let x2 = i64::max(x1 + MIN_APP_SIZE as i64, pointer.x);
            let y2 = i64::max(y1 + MIN_APP_SIZE as i64, pointer.y);
            app.rect = Rect::from_xyxy([x1, y1, x2, y2]);
        },

        AppsInteractionState::PieAppMenu { app_name, anchor } => {

            let app = apps.get_mut(app_name).unwrap();

            let entries = [
                PieMenuEntry::Button {
                    icon: &resources::CLOSE_ICON,
                    color: stylesheet.colors.red,
                    text: "Close".to_owned(),
                    text_color: stylesheet.colors.text,
                    font: &HACK_15,
                    weight: 1.0,
                },
                PieMenuEntry::Spacer {
                    color: stylesheet.colors.background,
                    weight: 1.0,
                },
                PieMenuEntry::Button {
                    icon: &resources::RELOAD_ICON,
                    color: stylesheet.colors.yellow,
                    text: "Reload".to_owned(),
                    text_color: stylesheet.colors.text,
                    font: &HACK_15,
                    weight: 1.0,
                },
                PieMenuEntry::Spacer {
                    color: stylesheet.colors.background,
                    weight: 3.0,
                },
            ];

            let (selected, draw_calls) = pie_menu(uitk_context, &entries, anchor);

            pie_draw_calls.replace(draw_calls);

            match selected {
                Some("Close") if pointer.left_click_trigger => app.is_open = false,
                _ => (),
            }

            if pointer.right_click_trigger || pointer.left_click_trigger {
                *is = AppsInteractionState::Idle
            }
        },

        AppsInteractionState::PieDesktopMenu { anchor } => {

            let entries: Vec<PieMenuEntry> = apps
                .values()
                .map(|app| PieMenuEntry::Button {
                    icon: app.descriptor.icon,
                    color: stylesheet.colors.background,
                    text: app.descriptor.name.to_owned(),
                    text_color: stylesheet.colors.text,
                    font: &HACK_15,
                    weight: 1.0,
                })
                .collect();

            let (selected, draw_calls) = pie_menu(uitk_context, &entries, anchor);

            pie_draw_calls.replace(draw_calls);

            match selected {
                Some(app_name) if pointer.left_click_trigger => {

                    let z_max = get_z_max(&apps);

                    let app = apps.get_mut(app_name).unwrap();

                    let deco = compute_decorations(app, input_state);

                    let preferred_rect =
                        Rect::from_center(pointer.x, pointer.y, app.rect.w, app.rect.h);

                    app.is_open = true;
                    app.rect = position_window(&preferred_rect, uitk_context.fb.shape(), &deco);
                    app.z_order = z_max + 1;
                }

                _ => (),
            }

            if pointer.right_click_trigger || pointer.left_click_trigger {
                *is = AppsInteractionState::Idle
            }
        }
    }

    normalize_z_order(apps);

    //
    // DEBUG

    //log::debug!("{:?}", is);

    //
    // Step and draw apps

    let UiContext { fb, .. } = uitk_context;

    for (app_name, app) in get_z_sorted_apps(apps) {

        if !app.is_open {
            continue;
        }

        let deco = compute_decorations(&app, input_state);
        let app_fb = app.wasm_app.step(system, input_state, &app.rect);

        let highlight = match *is {
            AppsInteractionState::AppHover { 
                app_name: hover_app_name,
                hover_kind
            } => hover_app_name == app_name && hover_kind == HoverKind::Titlebar,
            _ => false,
        };

        draw_app(*fb, &stylesheet, app_name, &app_fb, &deco, highlight);
    }

    if let Some(draw_calls) = pie_draw_calls {
        draw_calls.draw(*fb);
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

fn normalize_z_order(apps: &mut BTreeMap<&'static str, App>) {
    let mut sorted_apps = get_z_sorted_apps(apps);
    sorted_apps.iter_mut().enumerate().for_each(|(i, (_app_name, app))| app.z_order = i);
}

fn get_z_max(apps: &BTreeMap<&'static str, App>) -> usize {
    apps.values().map(|app| app.z_order).max().unwrap()
}

fn get_z_sorted_apps<'a>(apps: &'a mut BTreeMap<&'static str, App>) -> Vec<(&'static str, &'a mut App)> {
    let mut sorted_apps: Vec<(&'static str, &'a mut App)> = apps.iter_mut()
        .map(|(app_name, app)| (*app_name, app))
        .collect();
    sorted_apps.sort_by_key(|(_app_name, app)| app.z_order);
    sorted_apps
}

fn position_window(preferred_rect: &Rect, fb_shape: (u32, u32), deco: &AppDecorations) -> Rect {
    let (fb_w, fb_h) = fb_shape;
    let Rect {
        mut x0,
        mut y0,
        w,
        h,
    } = *preferred_rect;

    x0 = i64::max(0, x0);
    y0 = i64::max(deco.titlebar_rect.h as i64, y0);
    x0 = i64::min((fb_w - w - 1) as i64, x0);
    y0 = i64::min((fb_h - h - 1) as i64, y0);

    Rect { x0, y0, w, h }
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
        w: window_rect.w - RESIZE_HANDLE_GAP - RESIZE_HANDLE_LEN,
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
        border_rects: [left_border_rect, right_border_rect, bottom_border_rect],
        handle_rects: [handle_rect_1, handle_rect_2],
        resize_zone_rect,
    }
}

fn draw_app<F: FbViewMut>(
    fb: &mut F,
    stylesheet: &StyleSheet,
    app_name: &str,
    app_fb: &Framebuffer<BorrowedPixels>,
    deco: &AppDecorations,
    highlight: bool,
) {

    let color_deco = match highlight {
        true => stylesheet.colors.hover_overlay,
        false => stylesheet.colors.background,
    };

    draw_rect(fb, &deco.titlebar_rect, color_deco, false);
    for rect in deco.border_rects.iter() {
        draw_rect(fb, rect, color_deco, false);
    }

    let text_h = deco.titlebar_font.char_h as u32;

    let padding = (deco.titlebar_rect.h - text_h) / 2;
    let text_rect = Rect {
        x0: deco.titlebar_rect.x0 + padding as i64,
        y0: 0,
        w: deco.titlebar_rect.w - 2 * padding,
        h: text_h,
    }
    .align_to_rect_vert(&deco.titlebar_rect);

    let ellipsized_title = ellipsize_text(app_name, &deco.titlebar_font, text_rect.w);

    draw_str(
        fb,
        &ellipsized_title,
        text_rect.x0,
        text_rect.y0,
        &deco.titlebar_font,
        stylesheet.colors.text,
        None,
    );

    fb.copy_from_fb(app_fb, deco.content_rect.origin(), false);

    for rect in deco.handle_rects.iter() {
        draw_rect(fb, rect, stylesheet.colors.accent, false);
    }
}

fn ellipsize_text(txt: &str, font: &Font, max_len: u32) -> String {
    let max_chars = max_len as usize / font.char_w;

    if txt.len() <= max_chars {
        txt.to_owned()
    } else if max_chars < 3 {
        String::new()
    } else {
        let s = txt.chars().take(max_chars - 3).collect::<String>();
        format!("{}...", s)
    }
}
