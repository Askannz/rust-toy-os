use alloc::vec::Vec;

use crate::{Rect, Framebuffer};
use crate::input::{InputState, InputEvent};
use crate::drawing::text::{RichText, format_rich_lines, FormattedRichLines, draw_rich_slice};

pub struct ScrollableText {
    config: TextConfig,
    offset: usize,
    lines: FormattedRichLines,
}

impl ScrollableText {

    pub fn new(config: &TextConfig) -> Self {
        Self { config: config.clone(), offset: 0, lines: Vec::new() }
    }

    pub fn update(&mut self, input_state: &InputState, text: Option<RichText>) -> bool {

        let mut redraw = false;

        if let Some(text) = text {
            self.lines = format_rich_lines(&text, &self.config.rect);
            self.offset = get_autoscroll_offset(&self.config.rect, &self.lines);
            redraw = true;
        }

        if self.config.scrollable {

            let ps = &input_state.pointer;

            if self.config.rect.check_contains_point(ps.x, ps.y) {
                for event in input_state.events {
                    if let Some(InputEvent::Scroll { delta }) = event {
                        let offset = self.offset as i64 - delta;
                        self.offset = i64::max(0, offset) as usize;
                        redraw = true;
                    }
                }
            }
        }

        redraw
    }

    pub fn draw(&self, fb: &mut Framebuffer) {

        let Rect { x0, y0, h, .. } = self.config.rect;
        let h: i64 = h.into();

        let mut y = y0;
        for (rich_text, line_h) in self.lines.iter().skip(self.offset) {
            if y + line_h > y0 + h { break; }
            draw_rich_slice(fb, &rich_text, x0, y);
            y += line_h;
        }
    }
}

fn get_autoscroll_offset(rect: &Rect, lines: &FormattedRichLines) -> usize {

    let h: i64 = rect.h.into();

    let mut y = h;
    let mut offset = lines.len();

    for (_, line_h) in lines.iter().rev() {
        if y - line_h < 0 { break; }
        offset -= 1;
        y -= line_h;
    }

    offset
}

#[derive(Clone)]
pub struct TextConfig {
    pub rect: Rect,
    pub scrollable: bool,
}

impl Default for TextConfig {
    fn default() -> Self {
        TextConfig {
            rect: Rect { x0: 0, y0: 0, w: 100, h: 25 },
            scrollable: true,
        }
    }
}
