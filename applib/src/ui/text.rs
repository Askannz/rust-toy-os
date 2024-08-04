use alloc::borrow::ToOwned;
use alloc::string::String;
use alloc::vec::Vec;

use crate::drawing::primitives::draw_rect;
use crate::{Color, Framebuffer, Rect};
use crate::input::{InputState, InputEvent};
use crate::input::{Keycode, CHARMAP};
use crate::drawing::text::{draw_rich_slice, draw_str, format_rich_lines, Font, FormattedRichLines, RichText, HACK_15};


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


pub fn string_input(buffer: &mut String, caps: &mut bool, input_state: &InputState, allow_newline: bool) {

    let check_is_shift = |keycode| {
        keycode == Keycode::KEY_LEFTSHIFT || 
        keycode == Keycode::KEY_RIGHTSHIFT
    };
    input_state.events.iter().for_each(|&event| match event {
        Some(InputEvent::KeyPress { keycode }) if check_is_shift(keycode) => *caps = true,
        Some(InputEvent::KeyRelease { keycode }) if check_is_shift(keycode) => *caps = false,
        _ => ()
    });

    for event in input_state.events {

        match event {

            // Enter
            Some(InputEvent::KeyPress { keycode: Keycode::KEY_ENTER }) if allow_newline => { buffer.push('\n'); },

            // Backspace
            Some(InputEvent::KeyPress { keycode: Keycode::KEY_BACKSPACE }) => { buffer.pop(); },

            // Character input
            Some(InputEvent::KeyPress { keycode }) => {

                let new_char = CHARMAP
                    .get(&keycode)
                    .map(|(low_c, up_c)| if *caps { *up_c } else { *low_c })
                    .flatten();

                if let Some(new_char) = new_char {
                    buffer.push(new_char);
                }
            }

            _ => ()
        };
    }

}


pub fn editable_text(
    config: &EditableTextConfig,
    fb: &mut Framebuffer,
    buffer: &mut String,
    caps: &mut bool,
    input_state: &InputState
) {

    string_input(buffer, caps, input_state, false);

    let EditableTextConfig { font, color, bg_color, .. } = config;
    let Rect { x0, y0, .. } = config.rect;

    if let Some(bg_color) = bg_color {
        draw_rect(fb, &config.rect, *bg_color);
    }
    draw_str(fb, buffer, x0, y0, font, *color, None);
}


pub fn scrollable_text(config: &TextConfig, fb: &mut Framebuffer, input_state: &InputState, text: &RichText) {


    let lines = format_rich_lines(&text, &config.rect);
    let mut offset = get_autoscroll_offset(&config.rect, &lines);

    if config.scrollable {

        let ps = &input_state.pointer;

        if config.rect.check_contains_point(ps.x, ps.y) {
            for event in input_state.events {
                if let Some(InputEvent::Scroll { delta }) = event {
                    let new_offset = offset as i64 - delta;
                    offset = i64::max(0, new_offset) as usize;
                }
            }
        }
    }

    let Rect { x0, y0, h, .. } = config.rect;
    let h: i64 = h.into();

    let mut y = y0;
    for (rich_text, line_h) in lines.iter().skip(offset) {
        if y + line_h > y0 + h { break; }
        draw_rich_slice(fb, &rich_text, x0, y);
        y += line_h;
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


