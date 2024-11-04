use crate::drawing::primitives::draw_rect;
use crate::drawing::text::{draw_str, Font};
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

        let font = font_family.get_default();
        let &Font { char_h, char_w, .. } = font;
        let text_h = char_h as u32;
        let text_w = (config.text.len() * char_w) as u32;

        let (text_x0, text_y0) = match &config.icon {
            Some(icon) => {
                let (icon_w, icon_h) = icon.shape();

                let rect = Rect {
                    x0: 0, y0: 0,
                    w: icon_w + text_w,
                    h: u32::max(icon_h, text_h)
                }.align_to_rect(&config.rect);

                fb.copy_from_fb(*icon, (rect.x0, rect.y0), true);

                (rect.x0 + icon_w as i64, rect.y0)
            },

            None => {
                let rect = Rect { x0: 0, y0: 0, w: text_w, h: text_h }.align_to_rect(&config.rect);
                (rect.x0, rect.y0)
            }
        };

        draw_str(
            *fb,
            &config.text,
            text_x0,
            text_y0,
            font,
            colorsheet.text,
            None,
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
