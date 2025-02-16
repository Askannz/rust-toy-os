use crate::drawing::primitives::draw_rect;
use crate::drawing::text::{draw_line_in_rect, draw_str, Font, FontFamily, TextJustification};
use crate::uitk::{ContentId, UiContext};
use crate::{Color, FbView, FbViewMut, Framebuffer, OwnedPixels, Rect, StyleSheet};
use alloc::borrow::ToOwned;
use alloc::string::String;

impl<'a, F: FbViewMut> UiContext<'a, F> {

    pub fn button(&mut self, config: &ButtonConfig) -> bool {
        let mut active = false;
        self.button_inner(config, &mut active);
        active
    }

    pub fn button_toggle(&mut self, config: &ButtonConfig, active: &mut bool) {
        self.button_inner(config, active);
    }

    fn button_inner(&mut self, config: &ButtonConfig, active: &mut bool) {

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
                    true => ButtonState::Active,
                    false => ButtonState::Idle,
                }
            }
        };

        let content_id = ContentId::from_hash((
            state,
            &config.rect,
            &config.text,
            // TODO: icon hash
        ));

        let button_fb = tile_cache.fetch_or_create(content_id, self.time, || {
            render_button(stylesheet, font_family, config, state)
        });

        let Rect { x0, y0, .. } = config.rect;
        fb.copy_from_fb(button_fb, (x0, y0), false);
    }
}

fn render_button(
    stylesheet: &StyleSheet,
    font_family: &FontFamily,
    config: &ButtonConfig,
    state: ButtonState
) -> Framebuffer<OwnedPixels> {

    let Rect { w, h, .. } = config.rect;
    let mut button_fb = Framebuffer::new_owned(w, h);

    let colorsheet = &stylesheet.colors;

    let button_color = match state {
        ButtonState::Idle => colorsheet.element,
        ButtonState::Hover => colorsheet.hover_overlay,
        ButtonState::Active => colorsheet.selected_overlay,
    };

    draw_rect(&mut button_fb, &config.rect, colorsheet.background, false);

    let button_rect = {
        let m = stylesheet.margin as i64;
        let [x0, y0, x1, y1] = config.rect.zero_origin().as_xyxy();
        Rect::from_xyxy([x0+m, y0+m, x1-m, y1-m])
    };

    draw_rect(&mut button_fb, &button_rect, button_color, false);

    let text_x0 = match &config.icon {
        None => button_rect.x0,
        Some(icon) => {
            let (icon_w, icon_h) = icon.shape();
            let m = i64::max(0, (button_rect.h - icon_h) as i64 / 2);

            let icon_x0 = m;
            let icon_y0 = m;

            button_fb.copy_from_fb(*icon, (icon_x0, icon_y0), true);

            icon_x0 + icon_w as i64
        }
    };

    let text_rect = {
        let [_x0, y0, x1, y1] = button_rect.as_xyxy();
        Rect::from_xyxy([text_x0, y0, x1, y1])
    };

    let font = font_family.get_default();
    draw_line_in_rect(
        &mut button_fb, &config.text, &text_rect,
        font,
        colorsheet.text,
        TextJustification::Left
    );

    button_fb
}

#[derive(PartialEq, Hash, Clone, Copy)]
enum ButtonState {
    Idle,
    Hover,
    Active,
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
