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

use crate::content::UuidProvider;
use crate::uitk::text::string_input;

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

        let old_cursor = *cursor;
        string_input(buffer, input_state, false, cursor, *uuid_provider);

        let time_sec = (self.time as u64) / 1000;
        let cursor_visible = time_sec % 2 == 0 || *cursor != old_cursor;

        let colorsheet = &stylesheet.colors;
        let font = font_family.get_default();
        let text_color = colorsheet.text;

        let tile_content_id = ContentId::from_hash((
            config.rect.w,
            config.rect.h,
            addr_of!(font),
            text_color,
            cursor_visible,
            *cursor,
            buffer.get_id(),
        ));

        let tile_fb = tile_cache.fetch_or_create(tile_content_id, self.time, || {
            //
            // Draw text

            let EditableTextConfig {
                rect,
            } = config;

            let mut tile_fb = Framebuffer::new_owned(rect.w, rect.h);

            let buffer = buffer.as_ref();

            let text_h = font.char_h as u32;
            let text_w = (buffer.len() * font.char_w) as u32;
            let text_rect = Rect { 
                x0: stylesheet.margin as i64,
                y0: 0,
                w: text_w,
                h: text_h
            }.align_to_rect_vert(rect);
            let (text_x0, text_y0) = (text_rect.x0, text_rect.y0);

            draw_str(&mut tile_fb, buffer.as_ref(), text_x0, text_y0, font, text_color, None);

            //
            // Draw blinking cursor

            if cursor_visible {
                let cursor_rect = Rect {
                    x0: text_x0 + (*cursor * font.char_w) as i64,
                    y0: text_y0,
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
