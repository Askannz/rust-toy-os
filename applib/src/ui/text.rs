use crate::{Rect, Framebuffer};
use crate::input::{InputState, InputEvent};
use crate::drawing::text::{RichText, draw_rich_text};

pub struct ScrollableText {
    pub text: RichText,
    config: TextConfig,
    offset: usize,
}

impl ScrollableText {

    pub fn new(config: &TextConfig) -> Self {
        Self { text: RichText::new(), config: config.clone(), offset: 0 }
    }

    pub fn update(&mut self, input_state: &InputState) {

        let ps = &input_state.pointer;

        if self.config.rect.check_contains_point(ps.x, ps.y) {
            for event in input_state.events {
                if let Some(InputEvent::Scroll { delta }) = event {
                    let offset = self.offset as i64 - delta;
                    self.offset = i64::max(0, offset) as usize;
                }
            }
        }
    }

    pub fn draw(&self, fb: &mut Framebuffer) {
        draw_rich_text(fb, &self.text, &self.config.rect, self.offset);
    }
}

#[derive(Clone)]
pub struct TextConfig {
    pub rect: Rect,
}

impl Default for TextConfig {
    fn default() -> Self {
        TextConfig {
            rect: Rect { x0: 0, y0: 0, w: 100, h: 25 },
        }
    }
}
