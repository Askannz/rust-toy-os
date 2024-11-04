use alloc::borrow::ToOwned;
use alloc::collections::btree_map::BTreeMap;
use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use applib::input::PointerState;
use applib::{BorrowedPixels, StyleSheet};

use crate::shell::{pie_menu, PieDrawCalls, PieMenuEntry};
use applib::drawing::primitives::draw_rect;
use applib::drawing::text::{draw_str, Font};
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

        // In "toggle" mode, another click is required to get out of this mode
        toggle: bool,  
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
}

pub struct AppsManager {
    z_ordered: Vec<App>
}

pub struct App {
    pub app_state: AppState,
    pub descriptor: AppDescriptor,
    pub is_open: bool,
    pub rect: Rect,
    pub time_used: f64,
}

pub enum AppState {
    Init,
    Running { wasm_app: WasmApp },
    Paused { wasm_app: WasmApp },
    Crashed { error: anyhow::Error }
}

impl AppsManager {

    pub fn new(apps: Vec<App>) -> Self {
        Self { z_ordered: apps }
    }

    fn get_mut(&mut self, app_name: &'static str) -> &mut App {
        self.z_ordered.iter_mut().find(|app| app.descriptor.name == app_name).unwrap()
    }

    fn set_on_top(&mut self, app_name: &'static str) {
        let index = self.z_ordered.iter().position(|app| app.descriptor.name == app_name).unwrap();
        let app = self.z_ordered.remove(index);
        self.z_ordered.push(app);
    }
}

pub fn run_apps<F: FbViewMut>(
    uitk_context: &mut uitk::UiContext<F>,
    system: &mut System,
    wasm_engine: &WasmEngine,
    apps_manager: &mut AppsManager,
    input_state: &InputState,
    interaction_state: &mut AppsInteractionState,
) {
    const MIN_APP_SIZE: u32 = 200;

    let stylesheet = system.stylesheet.clone();
    let pointer = &input_state.pointer;
    let mut pie_draw_calls: Option<PieDrawCalls> = None;

    //
    // Hover

    let hover_state = apps_manager.z_ordered.iter().rev()
        .map(|app| {
            let deco = compute_decorations(app, input_state);
            (app, deco)
        })
        .find_map(|(app, deco)| {

            let app_name = app.descriptor.name;

            if !app.is_open { None }
            else if deco.titlebar_hover { Some((app_name, HoverKind::Titlebar)) }
            else if deco.resize_hover { Some((app_name, HoverKind::Resize)) }
            else if deco.window_hover { Some((app_name, HoverKind::Window)) }
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
            apps_manager.set_on_top(app_name);
            *is = AppsInteractionState::PieAppMenu { app_name, anchor };
        },

        AppsInteractionState::AppHover { app_name, hover_kind } if pointer.left_click_trigger => {

            apps_manager.set_on_top(app_name);

            match hover_kind {
                HoverKind::Titlebar => {
                    let app = apps_manager.get_mut(app_name);
                    let anchor = get_hold_anchor(pointer, &app.rect);
                    *is =  AppsInteractionState::TitlebarHold { app_name, anchor, toggle: false };
                },

                HoverKind::Resize => *is = AppsInteractionState::ResizeHold { app_name },

                HoverKind::Window => (),
            }
        },

        AppsInteractionState::AppHover { .. } => match hover_state {
            None => *is = AppsInteractionState::Idle,
            Some((app_name, hover_kind)) => *is = AppsInteractionState::AppHover { app_name, hover_kind }
        },

        AppsInteractionState::TitlebarHold { toggle, .. } if !toggle && !pointer.left_clicked => {
            *is = AppsInteractionState::Idle;
        },
        
        AppsInteractionState::TitlebarHold { toggle, .. } if toggle && (pointer.left_click_trigger ||  pointer.right_click_trigger) => {
            *is = AppsInteractionState::Idle;
        },

        AppsInteractionState::TitlebarHold { app_name, anchor, .. } => {
            let app = apps_manager.get_mut(app_name);
            app.rect.x0 = pointer.x - anchor.x;
            app.rect.y0 = pointer.y - anchor.y;
        },

        AppsInteractionState::ResizeHold { .. } if !pointer.left_clicked => {
            *is = AppsInteractionState::Idle;
        },

        AppsInteractionState::ResizeHold { app_name } => {
            let app = apps_manager.get_mut(app_name);
            let [x1, y1, _, _] = app.rect.as_xyxy();
            let x2 = i64::max(x1 + MIN_APP_SIZE as i64, pointer.x);
            let y2 = i64::max(y1 + MIN_APP_SIZE as i64, pointer.y);
            app.rect = Rect::from_xyxy([x1, y1, x2, y2]);
        },

        AppsInteractionState::PieAppMenu { app_name, anchor } => {

            let app = apps_manager.get_mut(app_name);

            let entries = [
                PieMenuEntry::Button {
                    icon: &resources::CLOSE_ICON,
                    color: stylesheet.colors.red,
                    text: "Close".to_owned(),
                    text_color: stylesheet.colors.text,
                    weight: 1.0,
                },
                PieMenuEntry::Button {
                    icon: &resources::MOVE_ICON,
                    color: stylesheet.colors.blue,
                    text: "Move".to_owned(),
                    text_color: stylesheet.colors.text,
                    weight: 1.0,
                },
                PieMenuEntry::Button {
                    icon: &resources::RELOAD_ICON,
                    color: stylesheet.colors.yellow,
                    text: "Reload".to_owned(),
                    text_color: stylesheet.colors.text,
                    weight: 1.0,
                },
                match app.app_state {
                    AppState::Running { .. } => PieMenuEntry::Button {
                        icon: &resources::PAUSE_ICON,
                        color: stylesheet.colors.yellow,
                        text: "Pause".to_owned(),
                        text_color: stylesheet.colors.text,
                        weight: 1.0,
                    },
                    AppState::Paused { .. } => PieMenuEntry::Button {
                        icon: &resources::PLAY_ICON,
                        color: stylesheet.colors.green,
                        text: "Resume".to_owned(),
                        text_color: stylesheet.colors.text,
                        weight: 1.0,
                    },
                    _ => PieMenuEntry::Spacer { 
                        color: stylesheet.colors.background,
                        weight: 1.0
                    },
                },
                PieMenuEntry::Spacer {
                    color: stylesheet.colors.background,
                    weight: 3.0,
                },
            ];

            let (selected, draw_calls) = pie_menu(uitk_context, &entries, anchor);

            pie_draw_calls.replace(draw_calls);

            match selected {
                Some(0) if pointer.left_click_trigger => {
                    app.is_open = false;
                    *is = AppsInteractionState::Idle;
                },
                Some(1) if pointer.left_click_trigger => {
                    let anchor = get_hold_anchor(pointer, &app.rect);
                    *is = AppsInteractionState::TitlebarHold { app_name, anchor, toggle: true };
                },
                Some(2) if pointer.left_click_trigger => {
                    log::info!("De-loading app {}", app.descriptor.name);
                    app.app_state = AppState::Init;
                    *is = AppsInteractionState::Idle;
                },
                Some(3) if pointer.left_click_trigger => {

                    // AppState::Init is just a placeholder for the swap
                    let tmp = core::mem::replace(&mut app.app_state, AppState::Init);

                    app.app_state = match tmp {
                        AppState::Running { wasm_app } => {
                            log::info!("Pausing app {}", app.descriptor.name);
                            *is = AppsInteractionState::Idle;
                            AppState::Paused { wasm_app }
                        },
                        AppState::Paused { wasm_app } => {
                            log::info!("Resuming app {}", app.descriptor.name);
                            *is = AppsInteractionState::Idle;
                            AppState::Running { wasm_app }
                        },
                        _ => tmp
                    }
                },
                _ if pointer.right_click_trigger || pointer.left_click_trigger => {
                    *is = AppsInteractionState::Idle;
                }
                _ => (),
            }
        },

        AppsInteractionState::PieDesktopMenu { anchor } => {

            let entries: Vec<PieMenuEntry> = apps_manager.z_ordered.iter()
                .map(|app| PieMenuEntry::Button {
                    icon: app.descriptor.icon,
                    color: stylesheet.colors.background,
                    text: app.descriptor.name.to_string(),
                    text_color: stylesheet.colors.text,
                    weight: 1.0,
                })
                .collect();

            let (selected, draw_calls) = pie_menu(uitk_context, &entries, anchor);

            pie_draw_calls.replace(draw_calls);

            match selected {
                Some(entry_index) if pointer.left_click_trigger => {

                    let app = &mut apps_manager.z_ordered[entry_index];

                    let deco = compute_decorations(app, input_state);

                    let preferred_rect =
                        Rect::from_center(pointer.x, pointer.y, app.rect.w, app.rect.h);

                    app.is_open = true;
                    app.rect = position_window(&preferred_rect, uitk_context.fb.shape(), &deco);
                    let app_name = app.descriptor.name;
                    apps_manager.set_on_top(app_name);
                }

                _ => (),
            }

            if pointer.right_click_trigger || pointer.left_click_trigger {
                *is = AppsInteractionState::Idle
            }
        }
    }

    //
    // DEBUG

    //log::debug!("{:?}", is);

    //
    // Step and draw apps

    let UiContext { fb, .. } = uitk_context;

    let font = uitk_context.font_family.get_default();

    let n = apps_manager.z_ordered.len();

    for (i, app) in apps_manager.z_ordered.iter_mut().enumerate() {

        if !app.is_open {
            continue;
        }

        let app_name = &app.descriptor.name;
        let deco = compute_decorations(&app, input_state);

        let highlight = match *is {
            AppsInteractionState::AppHover { 
                app_name: hover_app_name,
                hover_kind
            } => hover_app_name == *app_name && hover_kind == HoverKind::Titlebar,
            _ => false,
        };

        let is_foreground = i == n - 1;

        draw_decorations(*fb, &stylesheet, font, app_name, &deco, highlight);

        //fb.copy_from_fb(app_fb, deco.content_rect.origin(), false);
    
        match &mut app.app_state {

            AppState::Init => {

                let desc = &app.descriptor;

                log::info!("Initializing app {}", desc.name);
                let wasm_app = wasm_engine.instantiate_app(
                    system,
                    input_state,
                    desc.data,
                    desc.name,
                    &app.rect,
                );

                app.app_state = AppState::Running { wasm_app };
            },

            AppState::Running { wasm_app } => {
                
                let wasm_res = wasm_app.step(system, input_state, &app.rect, is_foreground);

                match wasm_res {
                    Ok(()) => if let Some(app_fb) = wasm_app.get_framebuffer() {
                        fb.copy_from_fb(&app_fb, deco.content_rect.origin(), false)
                    },
                    Err(error) => app.app_state = AppState::Crashed { error },
                }
            },

            AppState::Paused { wasm_app } => {
                if let Some(app_fb) = wasm_app.get_framebuffer() {
                    fb.copy_from_fb(&app_fb, deco.content_rect.origin(), false)
                }
                draw_rect(*fb, &deco.content_rect, Color::rgba(100, 100, 100, 100), true);
            },

            AppState::Crashed { error } => {

                let font = uitk_context.font_family.get_default();
                let (x0, y0) = deco.content_rect.origin();
                draw_str(
                    *fb,
                    &format!("{:?}", error),
                    x0, y0,
                    font,
                    Color::WHITE,
                    None
                );
            },
        }
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
    titlebar_hover: bool,
    resize_hover: bool,
    window_hover: bool,
}


fn get_hold_anchor(pointer: &PointerState, rect: &Rect) -> Point2D<i64> {
    let dx = pointer.x - rect.x0;
    let dy = pointer.y - rect.y0;
    Point2D { x: dx, y: dy }
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
        titlebar_hover,
        resize_hover,
        window_hover,
        border_rects: [left_border_rect, right_border_rect, bottom_border_rect],
        handle_rects: [handle_rect_1, handle_rect_2],
        resize_zone_rect,
    }
}

fn draw_decorations<F: FbViewMut>(
    fb: &mut F,
    stylesheet: &StyleSheet,
    font: &Font,
    app_name: &str,
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

    let text_h = font.char_h as u32;

    let padding = (deco.titlebar_rect.h - text_h) / 2;
    let text_rect = Rect {
        x0: deco.titlebar_rect.x0 + padding as i64,
        y0: 0,
        w: deco.titlebar_rect.w - 2 * padding,
        h: text_h,
    }
    .align_to_rect_vert(&deco.titlebar_rect);

    let ellipsized_title = ellipsize_text(app_name, font, text_rect.w);

    draw_str(
        fb,
        &ellipsized_title,
        text_rect.x0,
        text_rect.y0,
        font,
        stylesheet.colors.text,
        None,
    );

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
