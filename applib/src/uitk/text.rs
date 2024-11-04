use core::hash::Hash;
use core::ptr::addr_of;

use alloc::string::String;
use alloc::vec::Vec;

use crate::content::TrackedContent;
use crate::drawing::primitives::draw_rect;
use crate::drawing::text::{draw_rich_slice, draw_str, Font, FormattedRichText};
use crate::input::{InputEvent, InputState};
use crate::input::{Keycode, CHARMAP};
use crate::uitk::{ContentId, UiContext, CachedTile};
use crate::Framebuffer;
use crate::{Color, FbViewMut, Rect};

use super::UuidProvider;

#[derive(Clone)]
pub struct EditableTextConfig {
    pub rect: Rect,
}

impl Default for EditableTextConfig {
    fn default() -> Self {
        EditableTextConfig {
            rect: Rect {
                x0: 0,
                y0: 0,
                w: 100,
                h: 25,
            },
        }
    }
}

pub fn string_input(
    buffer: &mut TrackedContent<String>,
    input_state: &InputState,
    allow_newline: bool,
    cursor: &mut usize,
    uuid_provider: &mut UuidProvider,
) {
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
        let buffer = buffer.mutate(uuid_provider);
        for update in updates {
            match update {
                TextUpdate::Newline => {
                    buffer.insert(*cursor, '\n');
                    *cursor += 1;
                }
                TextUpdate::Backspace => {
                    if *cursor > 0 {
                        buffer.remove(*cursor - 1);
                        *cursor -= 1;
                    }
                }
                TextUpdate::Char(c) => {
                    buffer.insert(*cursor, c);
                    *cursor += 1;
                }
            }
        }
    }
}

impl<'a, F: FbViewMut> UiContext<'a, F> {
    pub fn editable_text(
        &mut self,
        config: &EditableTextConfig,
        buffer: &mut TrackedContent<String>,
        cursor: &mut usize,
    ) {
        let UiContext {
            fb,
            input_state,
            uuid_provider,
            tile_cache,
            font_family,
            stylesheet,
            ..
        } = self;

        string_input(buffer, input_state, false, cursor, *uuid_provider);

        let time_sec = (self.time as u64) / 1000;
        let cursor_visible = time_sec % 2 == 0;

        let colorsheet = &stylesheet.colors;
        let font = font_family.get_default();
        let text_color = colorsheet.text;

        let tile_content_id = ContentId::from_hash((
            config.rect.w,
            config.rect.h,
            addr_of!(font),
            text_color,
            cursor_visible,
            buffer.get_id(),
        ));

        let tile_fb = tile_cache.fetch_or_create(tile_content_id, self.time, || {
            //
            // Draw text

            let EditableTextConfig {
                rect,
            } = config;

            let mut tile_fb = Framebuffer::new_owned(rect.w, rect.h);

            draw_str(&mut tile_fb, buffer.as_ref(), 0, 0, font, text_color, None);

            //
            // Draw blinking cursor

            if cursor_visible {
                let cursor_rect = Rect {
                    x0: (*cursor * font.char_w) as i64,
                    y0: 0,
                    w: 2,
                    h: font.char_h as u32,
                };
                draw_rect(&mut tile_fb, &cursor_rect, text_color, false);
            }

            tile_fb
        });

        let Rect { x0, y0, .. } = config.rect;
        fb.copy_from_fb(tile_fb, (x0, y0), false);
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
