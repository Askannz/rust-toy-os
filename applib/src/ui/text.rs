use alloc::string::String;
use alloc::vec::Vec;

use crate::drawing::primitives::draw_rect;
use crate::{Color, Framebuffer, Rect};
use crate::input::{InputState, InputEvent};
use crate::input::{Keycode, CHARMAP};
use crate::drawing::text::{draw_rich_slice, draw_str, format_rich_lines, Font, FormattedRichLines, RichText, HACK_15};


pub struct EditableText {
    config: EditableTextConfig,
    text: String,
    is_shift_pressed: bool,
    is_flushed: bool,
}

#[derive(Clone)]
pub struct EditableTextConfig {
    pub rect: Rect,
    pub font: &'static Font,
    pub color: Color,
    pub bg_color: Option<Color>,
}

impl Default for EditableTextConfig {
    fn default() -> Self {
        EditableTextConfig {
            rect: Rect { x0: 0, y0: 0, w: 100, h: 25 },
            font: &HACK_15,
            color: Color::WHITE,
            bg_color: None,
        }
    }
}



impl EditableText {

    pub fn new(config: &EditableTextConfig) -> Self {
        Self { config: config.clone(), text: String::from(""), is_shift_pressed: false, is_flushed: false }
    }

    pub fn update(&mut self, input_state: &InputState) -> bool {

        let check_is_shift = |keycode| {
            keycode == Keycode::KEY_LEFTSHIFT || 
            keycode == Keycode::KEY_RIGHTSHIFT
        };
        input_state.events.iter().for_each(|&event| match event {
            Some(InputEvent::KeyPress { keycode }) if check_is_shift(keycode) => self.is_shift_pressed = true,
            Some(InputEvent::KeyRelease { keycode }) if check_is_shift(keycode) => self.is_shift_pressed = false,
            _ => ()
        });
    
    
        self.is_flushed = false;
        let mut redraw = false;
    
        for event in input_state.events {
    
            match event {
    
                // Enter key pressed (flushing input)
                Some(InputEvent::KeyPress { keycode: Keycode::KEY_ENTER }) => {
                    self.is_flushed = true;
                },
    
                // Backspace
                Some(InputEvent::KeyPress { keycode: Keycode::KEY_BACKSPACE }) => { 
                    self.text.pop();
                    redraw = true;
                },
    
                // Character input
                Some(InputEvent::KeyPress { keycode }) => {
    
                    let new_char = CHARMAP
                        .get(&keycode)
                        .map(|(low_c, up_c)| if self.is_shift_pressed { *up_c } else { *low_c })
                        .flatten();
    
                    if let Some(new_char) = new_char {
                        self.text.push(new_char);
                        redraw = true;
                    }
                }
    
                _ => ()
            };
        }

        redraw
    }

    pub fn is_flushed(&self) -> bool {
        return self.is_flushed
    }

    pub fn text(&self) -> &str {
        &self.text
    }

    pub fn draw(&mut self, fb: &mut Framebuffer) {

        let EditableTextConfig { font, color, bg_color, .. } = self.config;
        let Rect { x0, y0, .. } = self.config.rect;

        if let Some(bg_color) = bg_color {
            draw_rect(fb, &self.config.rect, bg_color);
        }
        draw_str(fb, &self.text, x0, y0, font, color, None);
    }
}


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


