use core::hash::Hash;
use core::ptr::addr_of;

use alloc::string::String;
use alloc::vec::Vec;

use crate::content::TrackedContent;
use crate::drawing::primitives::draw_rect;
use crate::drawing::text::{draw_rich_slice, draw_str, Font, FormattedRichText, RichText};
use crate::input::{InputEvent, InputState};
use crate::input::{Keycode, CHARMAP};
use crate::uitk::{ContentId, UiContext, CachedTile};
use crate::Framebuffer;
use crate::{Color, FbViewMut, Rect};

use super::UuidProvider;

pub fn string_input<T: EditableText>(
    buffer: &mut T,
    input_state: &InputState,
    allow_newline: bool,
    cursor: &mut usize,
    uuid_provider: &mut UuidProvider,
) {
    let buf_len = buffer.len();
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
            Some(InputEvent::KeyPress {
                keycode: Keycode::KEY_ENTER,
            }) if allow_newline => {
                updates.push(TextUpdate::Newline);
            }

            // Backspace
            Some(InputEvent::KeyPress {
                keycode: Keycode::KEY_BACKSPACE,
            }) => {
                updates.push(TextUpdate::Backspace);
            }

            // Cursor movement
            Some(InputEvent::KeyPress {
                keycode: Keycode::KEY_LEFT,
            }) if *cursor > 0 => {
                *cursor -= 1;
            }
            Some(InputEvent::KeyPress {
                keycode: Keycode::KEY_RIGHT,
            }) if *cursor < buf_len => {
                *cursor += 1;
            }

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

            _ => (),
        };
    }

    if !updates.is_empty() {
        for update in updates {
            match update {
                TextUpdate::Newline => {
                    buffer.insert(uuid_provider, *cursor, '\n');
                    *cursor += 1;
                }
                TextUpdate::Backspace => {
                    if *cursor > 0 {
                        buffer.remove(uuid_provider, *cursor - 1);
                        *cursor -= 1;
                    }
                }
                TextUpdate::Char(c) => {
                    buffer.insert(uuid_provider, *cursor, c);
                    *cursor += 1;
                }
            }
        }
    }
}

pub trait EditableText {
    fn len(&self) -> usize;
    fn insert(&mut self, uuid_provider: &mut UuidProvider, pos: usize, c: char);
    fn remove(&mut self, uuid_provider: &mut UuidProvider, pos: usize);
}

impl EditableText for TrackedContent<String> {

    fn len(&self) -> usize {
        self.as_ref().len()
    }

    fn insert(&mut self, uuid_provider: &mut UuidProvider, pos: usize, c: char) {
        self.mutate(uuid_provider).insert(pos, c);
    }

    fn remove(&mut self, uuid_provider: &mut UuidProvider, pos: usize) {
        self.mutate(uuid_provider).remove(pos);
    }
}

pub fn render_rich_text<F: FbViewMut>(
    dst_fb: &mut F,
    dst_rect: &Rect,
    formatted: &FormattedRichText,
    offsets: (i64, i64),
) {
    let Rect {
        x0: dst_x0,
        y0: dst_y0,
        h: dst_h,
        w: dst_w,
    } = *dst_rect;
    let (ox, oy) = offsets;

    let src_rect = Rect {
        x0: ox,
        y0: oy,
        w: dst_w,
        h: dst_h,
    };

    let mut y = 0;
    for line in formatted.lines.iter() {
        if y >= src_rect.y0 && y + (line.h as i64) <= src_rect.y0 + (src_rect.h as i64) {
            draw_rich_slice(dst_fb, &line.chars, dst_x0, dst_y0 + y - oy);
        }

        y += line.h as i64;
    }
}
