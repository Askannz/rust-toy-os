use alloc::borrow::ToOwned;
use alloc::string::String;
use crate::{Rect, Color, Framebuffer, PointerState};
use crate::drawing::text::{Font, draw_str, DEFAULT_FONT};
use crate::drawing::primitives::draw_rect;

pub struct Button {
    config: ButtonConfig,
    fired: bool,
}

impl Button {

    pub fn new(config: &ButtonConfig) -> Self {
        Self { config: config.clone(), fired: false }
    }

    pub fn update_and_draw(&mut self, fb: &mut Framebuffer, pointer_state: &PointerState) -> bool {
    
        let ps = pointer_state;

        let mut fire = false;
        let mut button_color = self.config.idle_color;

        if self.config.rect.check_contains_point(ps.x, ps.y) {
    
            if ps.left_clicked {

                if !self.fired { 
                    fire = true;
                    self.fired = true;
                }

                button_color = self.config.click_color;

            } else {
                self.fired = false;
                button_color = self.config.hover_color;
            }
        } else {
            self.fired = false;
        }

        let Rect { x0, y0, w, h } = self.config.rect;
        let &Font { char_w, char_h, .. } = self.config.font;

        let (w, h): (i64, i64) = (w.into(), h.into());
        let (char_w, char_h) = (char_w as i64, char_h as i64);
        let text_w = (self.config.text.len() as i64) * char_w;

        let text_x0 = x0 + i64::max(0, (w - text_w) / 2);
        let text_y0 = y0 + i64::max(0, (h - char_h) / 2);

        draw_rect(fb, &self.config.rect, button_color);
        draw_str(fb, &self.config.text, text_x0, text_y0, self.config.font, self.config.text_color);

        fire
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
    pub click_color: Color,
}

impl Default for ButtonConfig {
    fn default() -> Self {
        ButtonConfig {
            rect: Rect { x0: 0, y0: 0, w: 100, h: 25 },
            text: "Button".to_owned(),
            font: &DEFAULT_FONT,
            text_color: Color::hex(0xFFFFFF),
            idle_color: Color::hex(0x444444),
            hover_color: Color::hex(0x888888),
            click_color: Color::hex(0x222222),
        }
    }
}
