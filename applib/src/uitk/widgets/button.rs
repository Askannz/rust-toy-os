use crate::drawing::primitives::draw_rect;
use crate::drawing::text::{draw_str, Font, TextJustification, draw_line_in_rect};
use crate::uitk::{UiContext};
use crate::{Color, FbView, FbViewMut, Framebuffer, OwnedPixels, Rect};
use alloc::borrow::ToOwned;
use alloc::string::String;

impl<'a, F: FbViewMut> UiContext<'a, F> {
    pub fn button(&mut self, config: &ButtonConfig) -> bool {
        #[derive(PartialEq)]
        enum InteractionState {
            Idle,
            Hover,
            Clicked,
        }

        let UiContext {
            fb, input_state, stylesheet, font_family, ..
        } = self;

        let colorsheet = &stylesheet.colors;
        let ps = &input_state.pointer;

        let interaction_state = match config.rect.check_contains_point(ps.x, ps.y) {
            true => match ps.left_click_trigger {
                true => InteractionState::Clicked,
                false => InteractionState::Hover,
            },
            false => InteractionState::Idle,
        };

        let button_color = match interaction_state {
            InteractionState::Idle => colorsheet.element,
            InteractionState::Hover => colorsheet.hover_overlay,
            InteractionState::Clicked => colorsheet.selected_overlay,
        };

        draw_rect(*fb, &config.rect, colorsheet.background, false);

        let button_rect = {
            let m = stylesheet.margin as i64;
            let [x0, y0, x1, y1] = config.rect.as_xyxy();
            Rect::from_xyxy([x0+m, y0+m, x1-m, y1-m])
        };

        draw_rect(*fb, &button_rect, button_color, false);

        let text_x0 = match &config.icon {
            None => button_rect.x0,
            Some(icon) => {
                let (icon_w, icon_h) = icon.shape();
                let m = i64::max(0, (button_rect.h - icon_h) as i64 / 2);

                let icon_x0 = button_rect.x0 + m;
                let icon_y0 = button_rect.y0 + m;

                fb.copy_from_fb(*icon, (icon_x0, icon_y0), true);

                icon_x0 + icon_w as i64
            }
        };

        let text_rect = {
            let [_x0, y0, x1, y1] = button_rect.as_xyxy();
            Rect::from_xyxy([text_x0, y0, x1, y1])
        };

        let font = font_family.get_default();
        draw_line_in_rect(
            *fb, &config.text, &text_rect,
            font,
            colorsheet.text,
            TextJustification::Left
        );

        interaction_state == InteractionState::Clicked
    }
}

#[derive(Clone)]
pub struct ButtonConfig {
    pub rect: Rect,
    pub text: String,
    pub icon: Option<&'static Framebuffer<OwnedPixels>>,
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
        }
    }
}
