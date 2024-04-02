use alloc::borrow::ToOwned;
use alloc::string::String;
use crate::{Rect, Color, Framebuffer};
use crate::input::InputState;
use crate::drawing::text::{Font, draw_str, DEFAULT_FONT};
use crate::drawing::primitives::draw_rect;

pub struct Button {
    config: ButtonConfig,
    state: State,
    fired: bool,
}

impl Button {

    pub fn new(config: &ButtonConfig) -> Self {
        Self { config: config.clone(), state: State::Idle, fired: false }
    }

    pub fn update(&mut self, input_state: &InputState) -> bool {
    
        let ps = &input_state.pointer;

        self.fired = false;
        let mut new_state = self.state.clone();

        if self.config.rect.check_contains_point(ps.x, ps.y) {
            if ps.left_clicked {

                if new_state != State::Clicked { 
                    self.fired = true;
                    new_state = State::Clicked;
                }

            } else {
                new_state = State::Hover;
            }
        } else {
            new_state = State::Idle;
        }

        let mut redraw = false;
        if new_state != self.state {
            self.state = new_state;
            redraw = true;
        }

        redraw
    }

    pub fn is_fired(&self) -> bool {
        return self.fired
    }
    

    pub fn draw(&mut self, fb: &mut Framebuffer) {

        let button_color = match self.state {
            State::Idle => self.config.idle_color,
            State::Hover => self.config.hover_color,
            State::Clicked => self.config.clicked_color,
        };

        let Rect { x0, y0, h, .. } = self.config.rect;
        let h: i64 = h.into();
        let x_padding: i64 = self.config.x_padding.into();

        draw_rect(fb, &self.config.rect, button_color);

        let mut text_offset_x = 0;
        if let Some(icon) = self.config.icon {
            let (icon_w, icon_h) = (icon.w as u32, icon.h as u32);
            let icon_x0 = x0 + x_padding;
            let icon_y0 = y0 + i64::max(0, (h - i64::from(icon_h)) / 2);
            let src_rect = icon.shape_as_rect();
            let dst_rect = Rect { x0: icon_x0, y0: icon_y0, w: icon_w, h: icon_h };
            text_offset_x = icon.w as i64 + x_padding;
            fb.copy_from_fb(&icon, &src_rect, &dst_rect, true);
        }

        let &Font { char_h, .. } = self.config.font;
        let char_h = char_h as i64;

        let text_x0 = x0 + x_padding + text_offset_x;
        let text_y0 = y0 + i64::max(0, (h - char_h) / 2);

        draw_str(fb, &self.config.text, text_x0, text_y0, self.config.font, self.config.text_color, None);
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
    pub icon: Option<&'static Framebuffer<'static>>,
    pub x_padding: u32,
}

impl Default for ButtonConfig {
    fn default() -> Self {
        ButtonConfig {
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

#[derive(PartialEq, Clone)]
enum State {
    Idle,
    Hover,
    Clicked,
}
