use alloc::borrow::ToOwned;
use alloc::string::String;
use alloc::vec::Vec;

use crate::drawing::primitives::draw_rect;
use crate::{Color, Framebuffer, Rect};
use crate::input::{InputState, InputEvent};
use crate::input::{Keycode, CHARMAP};
use crate::drawing::text::{draw_rich_slice, draw_str, format_rich_lines, Font, RichText, HACK_15};
use super::{TrackedContent, ContentId};

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


pub fn string_input(buffer: &mut TrackedContent<String>, caps: &mut bool, input_state: &InputState, allow_newline: bool) {

    enum TextUpdate {
        Newline,
        Backspace,
        Char(char),
    }

    let check_is_shift = |keycode| {
        keycode == Keycode::KEY_LEFTSHIFT || 
        keycode == Keycode::KEY_RIGHTSHIFT
    };
    input_state.events.iter().for_each(|&event| match event {
        Some(InputEvent::KeyPress { keycode }) if check_is_shift(keycode) => *caps = true,
        Some(InputEvent::KeyRelease { keycode }) if check_is_shift(keycode) => *caps = false,
        _ => ()
    });

    let mut updates = Vec::new();

    for event in input_state.events {

        match event {

            // Enter
            Some(InputEvent::KeyPress { keycode: Keycode::KEY_ENTER }) if allow_newline => { /*buffer.push('\n')*/updates.push(TextUpdate::Newline); },

            // Backspace
            Some(InputEvent::KeyPress { keycode: Keycode::KEY_BACKSPACE }) => { /*buffer.pop()*/updates.push(TextUpdate::Backspace); },

            // Character input
            Some(InputEvent::KeyPress { keycode }) => {

                let new_char = CHARMAP
                    .get(&keycode)
                    .map(|(low_c, up_c)| if *caps { *up_c } else { *low_c })
                    .flatten();

                if let Some(new_char) = new_char {
                    /*buffer.push(new_char);*/
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
                TextUpdate::Newline => buffer.push('\n'),
                TextUpdate::Backspace => { buffer.pop(); },
                TextUpdate::Char(c) => buffer.push(c),
            }
        }
    }

}


pub fn editable_text(
    config: &EditableTextConfig,
    fb: &mut Framebuffer,
    buffer: &mut TrackedContent<String>,
    caps: &mut bool,
    input_state: &InputState
) {

    string_input(buffer, caps, input_state, false);

    let EditableTextConfig { font, color, bg_color, .. } = config;
    let Rect { x0, y0, .. } = config.rect;

    if let Some(bg_color) = bg_color {
        draw_rect(fb, &config.rect, *bg_color);
    }
    draw_str(fb, buffer.as_ref(), x0, y0, font, *color, None);
}

pub fn render_rich_text(text_fb: &mut Framebuffer, text: &RichText) {

    let max_w = text_fb.w;

    let formatted = format_rich_lines(text, max_w);
    
    text_fb.resize(max_w, formatted.h);
    text_fb.fill(Color::BLACK);

    let rect = text_fb.shape_as_rect();

    let Rect { x0, y0, h, .. } = rect;
    let h: i64 = h.into();

    let mut y = y0;
    for line in formatted.lines.iter() {
        if y + line.h as i64 > y0 + h { break; }
        draw_rich_slice(text_fb, &line.chars, x0, y);
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


