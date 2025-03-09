use crate::drawing::primitives::{draw_rect, draw_rect_outline};
use crate::drawing::text::{self, draw_line_in_rect, draw_str, Font, FontFamily, TextJustification};
use crate::uitk::{ContentId, UiContext};
use crate::{Color, FbView, FbViewMut, Framebuffer, OwnedPixels, Rect, StyleSheet};
use alloc::borrow::ToOwned;
use alloc::string::String;

impl<'a, F: FbViewMut> UiContext<'a, F> {

    pub fn button(&mut self, config: &ButtonConfig) -> bool {
        let mut active = false;
        self.button_inner(config, &mut active, false);
        active
    }

    pub fn button_toggle(&mut self, config: &ButtonConfig, active: &mut bool) {
        self.button_inner(config, active, true);
    }

    fn button_inner(&mut self, config: &ButtonConfig, active: &mut bool, indicator_visible: bool) {

        let UiContext {
            fb, input_state, stylesheet, font_family, tile_cache, ..
        } = self;

        let ps = &input_state.pointer;
        let hovered = !config.freeze && config.rect.check_contains_point(ps.x, ps.y);
        let clicked = hovered && ps.left_click_trigger;

        let state = {
            if hovered && !clicked {
                ButtonState::Hover
            } else {
                if hovered && clicked {
                    *active = !(*active);
                }
                match *active {
                    true => ButtonState::Clicked,
                    false => ButtonState::Idle,
                }
            }
        };

        let content_id = ContentId::from_hash(&(
            state,
            &config.rect,
            &config.text,
            *active,
            // TODO: icon hash
        ));

        let button_fb = tile_cache.fetch_or_create(content_id, self.time, || {
            render_button(stylesheet, font_family, config, state, *active, indicator_visible)
        });

        let Rect { x0, y0, .. } = config.rect;
        fb.copy_from_fb(button_fb, (x0, y0), false);
    }
}

fn render_button(
    stylesheet: &StyleSheet,
    font_family: &FontFamily,
    config: &ButtonConfig,
    state: ButtonState,
    active: bool,
    indicator_visible: bool
) -> Framebuffer<OwnedPixels> {

    let rect = config.rect.zero_origin();

    let Rect { w, h, .. } = rect;
    let mut button_fb = Framebuffer::new_owned(w, h);

    let colorsheet = &stylesheet.colors;

    draw_rect_outline(&mut button_fb, &rect, colorsheet.outline, false, stylesheet.margin);

    let button_rect = rect.offset(-(stylesheet.margin as i64));

    draw_rect(&mut button_fb, &button_rect, colorsheet.element, false);

    let mut x = button_rect.x0;

    if indicator_visible {

        let indicator_h = 3 * button_rect.h / 4;
        let indicator_w = 10;
        let margin = (button_rect.h - indicator_h) / 2;

        x += margin as i64;

        let indicator_rect = Rect {
            x0: x, y0: button_rect.y0 + margin as i64,
            w: indicator_w, h: indicator_h
        };
        let color = match active {
            true => colorsheet.accent,
            false => colorsheet.background
        };
        draw_rect(&mut button_fb, &indicator_rect, color, false);

        x += indicator_w  as i64;
    }

    let content_rect = {
        let [_, y0, x1, y1] = button_rect.as_xyxy();
        Rect::from_xyxy([x, y0, x1, y1])
    };

    if let Some(icon) = &config.icon {
        let (icon_w, icon_h) = icon.shape();

        let mut icon_rect = Rect {
            x0: content_rect.x0, y0: content_rect.y0,
            w: icon_w, h: icon_h,
        }.align_to_rect_vert(&content_rect);

        if config.text.is_empty() {
            icon_rect = icon_rect.align_to_rect_horiz(&content_rect);
        }

        button_fb.copy_from_fb(*icon, (icon_rect.x0, icon_rect.y0), true);

        x += icon_w as i64
    }

    let text_rect = Rect {
        x0: x, y0: button_rect.y0,
        w: button_rect.w, h: button_rect.h,
    };

    let font = font_family.get_default();
    draw_line_in_rect(
        &mut button_fb, &config.text, &text_rect,
        font,
        colorsheet.text,
        TextJustification::Left
    );

    if state == ButtonState::Hover {
        draw_rect(&mut button_fb, &button_rect, colorsheet.hover_overlay, true);
    }

    button_fb
}

#[derive(PartialEq, Hash, Clone, Copy)]
enum ButtonState {
    Idle,
    Hover,
    Clicked,
}

#[derive(Clone)]
pub struct ButtonConfig {
    pub rect: Rect,
    pub text: String,
    pub icon: Option<&'static Framebuffer<OwnedPixels>>,
    pub freeze: bool,
}

impl Default for ButtonConfig {
    fn default() -> Self {
        ButtonConfig {
            rect: Rect {
                x0: 0,
                y0: 0,
                w: 100,
                h: 25,
            },
            text: "Button".to_owned(),
            icon: None,
            freeze: false,
        }
    }
}
