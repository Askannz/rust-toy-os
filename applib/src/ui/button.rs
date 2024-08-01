use alloc::borrow::ToOwned;
use alloc::string::String;
use crate::{Rect, Color, Framebuffer};
use crate::input::InputState;
use crate::drawing::text::{Font, draw_str, DEFAULT_FONT};
use crate::drawing::primitives::draw_rect;

pub struct Button;

impl Button {

    pub fn update(fb: &mut Framebuffer, input_state: &InputState, button_state: &ButtonState) -> bool {

        #[derive(PartialEq)]
        enum InteractionState { Idle, Hover, Clicked }
    
        let ps = &input_state.pointer;

        let interaction_state = match button_state.rect.check_contains_point(ps.x, ps.y) {
            true => match ps.left_clicked {
                true => InteractionState::Clicked,
                false => InteractionState::Hover,
            },
            false => InteractionState::Idle
        };


        let button_color = match interaction_state {
            InteractionState::Idle => button_state.idle_color,
            InteractionState::Hover => button_state.hover_color,
            InteractionState::Clicked => button_state.clicked_color,
        };

        let Rect { x0, y0, h, .. } = button_state.rect;
        let h: i64 = h.into();
        let x_padding: i64 = button_state.x_padding.into();

        draw_rect(fb, &button_state.rect, button_color);

        let mut text_offset_x = 0;
        if let Some(icon) = button_state.icon {
            let (icon_w, icon_h) = (icon.w as u32, icon.h as u32);
            let icon_x0 = x0 + x_padding;
            let icon_y0 = y0 + i64::max(0, (h - i64::from(icon_h)) / 2);
            let src_rect = icon.shape_as_rect();
            let dst_rect = Rect { x0: icon_x0, y0: icon_y0, w: icon_w, h: icon_h };
            text_offset_x = icon.w as i64 + x_padding;
            fb.copy_from_fb(&icon, &src_rect, &dst_rect, true);
        }

        let &Font { char_h, .. } = button_state.font;
        let char_h = char_h as i64;

        let text_x0 = x0 + x_padding + text_offset_x;
        let text_y0 = y0 + i64::max(0, (h - char_h) / 2);

        draw_str(fb, &button_state.text, text_x0, text_y0, button_state.font, button_state.text_color, None);

        interaction_state == InteractionState::Clicked
    }
}

#[derive(Clone)]
pub struct ButtonState {
    pub rect: Rect,
    pub text: String,
    pub font: &'static Font,
    pub text_color: Color,
    pub idle_color: Color,
    pub hover_color: Color,
    pub clicked_color: Color,
    pub icon: Option<&'static Framebuffer<'static>>,
    pub x_padding: u32,
}

impl Default for ButtonState {
    fn default() -> Self {
        ButtonState {
            rect: Rect { x0: 0, y0: 0, w: 100, h: 25 },
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

