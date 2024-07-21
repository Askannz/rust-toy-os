use alloc::borrow::ToOwned;
use alloc::string::String;
use crate::{Rect, Color, Framebuffer};
use crate::drawing::text::{Font, draw_str, DEFAULT_FONT};
use crate::drawing::primitives::draw_rect;

pub struct ProgressBar {
    config: ProgressBarConfig,
    progress: u64,
    text: String
}

impl ProgressBar {

    pub fn new(config: &ProgressBarConfig) -> Self {
        Self { 
            config: config.clone(),
            progress: 0,
            text: "".to_owned(),
        }
    }

    pub fn update(&mut self, progress: u64, text: &str) -> bool {

        let mut redraw = false;
        
        if progress != self.progress {
            self.progress = progress;
            redraw = true;
        }

        if text != self.text.as_str() {
            self.text = text.to_owned();
            redraw = true;
        }

        redraw
    }
    

    pub fn draw(&mut self, fb: &mut Framebuffer) {

        let Rect { x0, y0, h, w } = self.config.rect;
        draw_rect(fb, &self.config.rect, self.config.bg_color);

        let p = self.config.bar_padding;
        let bar_w = (((w - 2*p) as u64) * self.progress / self.config.max_val) as u32;
        let bar_rect = Rect { 
            x0: x0 + p as i64,
            y0: y0 + p as i64,
            h: h - 2* p,
            w: bar_w
        };

        draw_rect(fb, &bar_rect, self.config.bar_color);

        let text_x_padding: i64 = self.config.text_x_padding.into();
        let &Font { char_h, .. } = self.config.font;
        let char_h = char_h as i64;
        let h: i64 = h.into();

        let text_x0 = x0 + text_x_padding;
        let text_y0 = y0 + i64::max(0, (h - char_h) / 2);

        draw_str(fb, &self.text, text_x0, text_y0, self.config.font, self.config.text_color, None);
    }
}

#[derive(Clone)]
pub struct ProgressBarConfig {
    pub rect: Rect,
    pub font: &'static Font,
    pub bg_color: Color,
    pub text_color: Color,
    pub bar_color: Color,
    pub bar_padding: u32,
    pub text_x_padding: u32,
    pub max_val: u64,
}

impl Default for ProgressBarConfig {
    fn default() -> Self {
        ProgressBarConfig {
            rect: Rect { x0: 0, y0: 0, w: 100, h: 25 },
            font: &DEFAULT_FONT,
            bg_color: Color::BLACK,
            text_color: Color::WHITE,
            bar_color: Color::BLUE,
            bar_padding: 2,
            text_x_padding: 10,
            max_val: 100,
        }
    }
}
