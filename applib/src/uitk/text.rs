use alloc::borrow::ToOwned;
use alloc::string::String;
use alloc::vec::Vec;

use crate::drawing::primitives::draw_rect;
use crate::{Color, Framebuffer, Rect};
use crate::input::{InputState, InputEvent};
use crate::input::{Keycode, CHARMAP};
use crate::drawing::text::{draw_rich_slice, draw_str, format_rich_lines, Font, RichText, FormattedRichText, HACK_15};
use crate::content::{TrackedContent, ContentId, UuidProvider};

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


pub fn string_input<P: UuidProvider>(buffer: &mut TrackedContent<P, String>, input_state: &InputState, allow_newline: bool, cursor: &mut usize) {

    let buf_len = buffer.as_ref().len();
    *cursor = usize::min(buf_len, *cursor);

    enum TextUpdate {
        Newline,
        Backspace,
        Char(char),
    }

    let mut updates = Vec::new();

    for event in input_state.events {

        match event {

            // Enter
            Some(InputEvent::KeyPress { keycode: Keycode::KEY_ENTER }) if allow_newline => { updates.push(TextUpdate::Newline); },

            // Backspace
            Some(InputEvent::KeyPress { keycode: Keycode::KEY_BACKSPACE }) => { updates.push(TextUpdate::Backspace); },

            // Cursor movement
            Some(InputEvent::KeyPress { keycode: Keycode::KEY_LEFT }) if *cursor > 0 => { *cursor -= 1; },
            Some(InputEvent::KeyPress { keycode: Keycode::KEY_RIGHT }) if *cursor < buf_len => { *cursor += 1; },

            // Character input
            Some(InputEvent::KeyPress { keycode }) => {

                let new_char = CHARMAP
                    .get(&keycode)
                    .map(|(low_c, up_c)| if input_state.shift { *up_c } else { *low_c })
                    .flatten();

                if let Some(new_char) = new_char {
                    updates.push(TextUpdate::Char(new_char))
                }
            }

            _ => ()
        };
    }

    if !updates.is_empty() {
        let buffer = buffer.mutate();
        for update in updates {
            match update {
                TextUpdate::Newline => {
                    buffer.insert(*cursor, '\n');
                    *cursor += 1;
                },
                TextUpdate::Backspace => {
                    if *cursor > 0 {
                        buffer.remove(*cursor - 1);
                        *cursor -= 1;
                    }
                },
                TextUpdate::Char(c) => {
                    buffer.insert(*cursor, c);
                    *cursor += 1;
                },
            }
        }
    }

}


pub fn editable_text<P: UuidProvider>(
    config: &EditableTextConfig,
    fb: &mut Framebuffer,
    buffer: &mut TrackedContent<P, String>,
    cursor: &mut usize,
    input_state: &InputState,
    time: f64,
) {

    let original_cursor = *cursor;
    let original_content_id = buffer.get_id();

    string_input(buffer, input_state, false, cursor);

    let EditableTextConfig { font, color, bg_color, .. } = config;
    let Rect { x0, y0, .. } = config.rect;

    if let Some(bg_color) = bg_color {
        draw_rect(fb, &config.rect, *bg_color);
    }
    draw_str(fb, buffer.as_ref(), x0, y0, font, *color, None);

    let time_sec = (time as u64) / 1000;
    if time_sec % 2 == 0 || buffer.get_id() != original_content_id || *cursor != original_cursor {
        let cursor_rect = Rect {
            x0: x0 + (*cursor * font.char_w) as i64,
            y0: y0,
            w: 2,
            h: font.char_h as u32,
        };
        draw_rect(fb, &cursor_rect, *color);
    }
}

pub fn render_rich_text(dst_fb: &mut Framebuffer, dst_rect: &Rect, formatted: &FormattedRichText, offsets: (i64, i64)) {

    let Rect { x0: dst_x0, y0: dst_y0, h: dst_h, w: dst_w } = *dst_rect;
    let (ox, oy) = offsets;

    let src_rect = Rect { x0: ox, y0: oy, w: dst_w, h:dst_h };

    let mut y = 0;
    for line in formatted.lines.iter() {

        if y >= src_rect.y0 && y + (line.h as i64) <= src_rect.y0 + (src_rect.h as i64) {
            draw_rich_slice(dst_fb, &line.chars, dst_x0, dst_y0 + y - oy);
        }
        
        y += line.h as i64;
    }
}


#[derive(Clone)]
pub struct ScrollableTextConfig {
    pub rect: Rect,
    pub scrollable: bool,
}

impl Default for ScrollableTextConfig {
    fn default() -> Self {
        ScrollableTextConfig {
            rect: Rect { x0: 0, y0: 0, w: 100, h: 25 },
            scrollable: true,
        }
    }
}


