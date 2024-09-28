use crate::drawing::primitives::draw_rect;
use crate::drawing::text::{draw_str, Font, DEFAULT_FONT};
use crate::uitk::UiContext;
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
            fb, input_state, ..
        } = self;

        let ps = &input_state.pointer;

        let interaction_state = match config.rect.check_contains_point(ps.x, ps.y) {
            true => match ps.left_click_trigger {
                true => InteractionState::Clicked,
                false => InteractionState::Hover,
            },
            false => InteractionState::Idle,
        };

        let button_color = match interaction_state {
            InteractionState::Idle => config.idle_color,
            InteractionState::Hover => config.hover_color,
            InteractionState::Clicked => config.clicked_color,
        };

        let Rect { x0, y0, h, .. } = config.rect;
        let h: i64 = h.into();
        let x_padding: i64 = config.x_padding.into();

        draw_rect(*fb, &config.rect, button_color, false);

        let mut text_offset_x = 0;
        if let Some(icon) = config.icon {
            let (icon_w, icon_h) = icon.shape();
            let icon_x0 = x0 + x_padding;
            let icon_y0 = y0 + i64::max(0, (h - i64::from(icon_h)) / 2);
            text_offset_x = icon_w as i64 + x_padding;
            fb.copy_from_fb(icon, (icon_x0, icon_y0), true);
        }

        let &Font { char_h, .. } = config.font;
        let char_h = char_h as i64;

        let text_x0 = x0 + x_padding + text_offset_x;
        let text_y0 = y0 + i64::max(0, (h - char_h) / 2);

        draw_str(
            *fb,
            &config.text,
            text_x0,
            text_y0,
            config.font,
            config.text_color,
            None,
        );

        interaction_state == InteractionState::Clicked
    }
}

#[derive(Clone)]
pub struct ButtonConfig {
    pub rect: Rect,
    pub text: String,
    pub font: &'static Font,
    pub text_color: Color,
    pub idle_color: Color,
    pub hover_color: Color,
    pub clicked_color: Color,
    pub icon: Option<&'static Framebuffer<OwnedPixels>>,
    pub x_padding: u32,
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
            font: &DEFAULT_FONT,
            text_color: Color::from_u32(0xFFFFFF),
            idle_color: Color::from_u32(0x444444),
            hover_color: Color::from_u32(0x888888),
            clicked_color: Color::from_u32(0x222222),
            icon: None,
            x_padding: 10,
        }
    }
}
