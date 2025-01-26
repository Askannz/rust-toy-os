use alloc::vec::Vec;
use alloc::string::String;

use crate::drawing::primitives::draw_rect;
use crate::drawing::text::{draw_rich_slice, format_rich_lines, Font, FormattedRichText, RichChar, RichText};
use crate::input::InputEvent;
use crate::input::{InputState};
use crate::content::{ContentId, TrackedContent};
use crate::{Color, StyleSheet};
use crate::Rect;
use crate::{BorrowedMutPixels, FbViewMut, FbView, Framebuffer};

use crate::uitk::{CachedTile, TileCache, TileRenderer, UiContext};

use crate::uitk::UuidProvider;
use crate::uitk::text::string_input;

impl<'a, F: FbViewMut> UiContext<'a, F> {

    pub fn text_box<T: FormattableText>(
        &mut self,
        dst_rect: &Rect,
        text: &mut T,
        state: &mut TextBoxState,
        editable: bool,
        autoscroll: bool,
        allow_newline: bool,
        prelude: Option<&T>,
    ) {

        let UiContext {
            input_state,
            uuid_provider,
            ..
        } = self;

        let old_cursor = state.cursor;

        if editable {
            text.string_input(input_state, allow_newline, &mut state.cursor, *uuid_provider);
        }

        let time_sec = (self.time as u64) / 1000;
        let cursor_visible = editable && (time_sec % 2 == 0 || state.cursor != old_cursor);

        // Only used if text is not already a RichText
        let font = self.font_family.get_default();
        let color = self.stylesheet.colors.text;

        let (rich_text, prelude_len) = match prelude {
            None => {
                let rich = text.to_rich_text(color, font);
                (rich, 0)
            },
            Some(prelude) => {
                let (mut rich_1, cid_1) = prelude.to_rich_text(color, font).to_inner();
                let (rich_2, cid_2) = text.to_rich_text(color, font).to_inner();

                let prelude_len = rich_1.len();
                
                rich_1.concat(rich_2);
                let cid = ContentId::from_hash((cid_1, cid_2));
                let rich = TrackedContent::new_with_id(rich_1, cid);
                
                (rich, prelude_len)
            }
        };

        let formatted = {
            let formatted = format_rich_lines(rich_text.as_ref(), dst_rect.w - CURSOR_W);
            let content_id = ContentId::from_hash((
                rich_text.get_id(),
                dst_rect.w,
            ));
            TrackedContent::new_with_id(formatted, content_id)
        };

        let formatted_content_id = formatted.get_id();

        let renderer = TextRenderer { 
            formatted, bg_color: self.stylesheet.colors.element,
            cursor: state.cursor, cursor_visible,
            prelude_len,
        };

        if autoscroll {
            let TextBoxState { content_id, scroll_offsets, .. } = state;
            match content_id {
                Some(content_id) if *content_id == formatted_content_id => (),
                _ => {
                    let (_, scroll_y0) = scroll_offsets;
                    let (_, max_h) = renderer.shape();
                    *scroll_y0 = (max_h - dst_rect.h - 1).into();
                    *content_id = Some(formatted_content_id);
                }
            }
        }

        self.dynamic_canvas(
            dst_rect,
            &renderer,
            &mut state.scroll_offsets,
            &mut state.scroll_dragging,
        );

        
    }

}

pub struct TextBoxState {
    pub content_id: Option<ContentId>,
    pub scroll_offsets: (i64, i64),
    pub scroll_dragging: (bool, bool),
    pub cursor: usize,
}

impl TextBoxState {
    pub fn new() -> Self {
        Self { 
            content_id: None,
            scroll_offsets: (0, 0),
            scroll_dragging: (false, false),
            cursor: 0,
        }
    }
}

pub trait FormattableText {
    fn to_rich_text(&self, color: Color, font: &'static Font) -> TrackedContent<RichText>;
    fn string_input(
        &mut self,
        input_state: &InputState,
        allow_newline: bool,
        cursor: &mut usize,
        uuid_provider: &mut UuidProvider,
    );
}

impl FormattableText for TrackedContent<String> {
    fn to_rich_text(&self, color: Color, font: &'static Font) -> TrackedContent<RichText> {
        let rich_text = RichText::from_str(self.as_ref(), color, font);
        let new_id = ContentId::from_hash((
            self.get_id(),
            color,
            font.name,
        ));
        TrackedContent::new_with_id(rich_text, new_id)
    }

    fn string_input(
            &mut self,
            input_state: &InputState,
            allow_newline: bool,
            cursor: &mut usize,
            uuid_provider: &mut UuidProvider,
        ) {
        
        string_input(self, input_state, allow_newline, cursor, uuid_provider);
    }
}

impl FormattableText for TrackedContent<RichText> {
    fn to_rich_text(&self, _color: Color, _font: &'static Font) -> TrackedContent<RichText> {
        let content_id = self.get_id();
        let rich_text = self.as_ref().clone();
        TrackedContent::new_with_id(rich_text, content_id)
    }

    fn string_input(
        &mut self,
        _input_state: &InputState,
        _allow_newline: bool,
        _cursor: &mut usize,
        _uuid_provider: &mut UuidProvider,
    ) {
    
        unimplemented!("string_input() not implemented for RichText");
    }
}

struct TextRenderer {
    formatted: TrackedContent<FormattedRichText>,
    bg_color: Color,
    cursor: usize,
    prelude_len: usize,
    cursor_visible: bool,
}

const CURSOR_W: u32 = 2;
const CURSOR_H: u32 = 20;
const MIN_TILE_W: u32 = 200;
const TILE_H: u32 = 200;

impl TileRenderer for TextRenderer {

    fn shape(&self) -> (u32, u32) {

        let FormattedRichText { w, h, .. } = *self.formatted.as_ref();
        (w + CURSOR_W, h)
    }

    fn tile_shape(&self) -> (u32, u32) {

        let FormattedRichText { w, .. } = *self.formatted.as_ref();
        (
            u32::max(w + CURSOR_W, MIN_TILE_W),
            TILE_H
        )
    }

    fn content_id(&self, tile_rect: &Rect) -> ContentId {

        let FormattedRichText { w, h, .. } = *self.formatted.as_ref();
        let text_rect = Rect { x0: 0, y0: 0, w, h: h + CURSOR_H};

        if tile_rect.intersection(&text_rect).is_none() {
            ContentId::from_hash((tile_rect.w, tile_rect.h))
        } else {
            ContentId::from_hash((
                tile_rect,
                self.formatted.get_id(),
                self.cursor,
                self.cursor_visible,
            ))
        }
    }

    fn render<F: FbViewMut>(&self, dst_fb: &mut F, tile_rect: &Rect) {

        let Rect { x0: ox, y0: oy, .. } = *tile_rect;

        dst_fb.fill(self.bg_color);

        if ox != 0 {
            return;
        }

        let mut y = 0;
        for line in self.formatted.as_ref().lines.iter() {
            // Bounding box of line in source
            let line_rect = Rect {
                x0: 0,
                y0: y,
                w: line.w,
                h: line.h,
            };

            if tile_rect.intersection(&line_rect).is_some() {
                draw_rich_slice(dst_fb, &line.chars, 0, y - oy);
            }

            y += line.h as i64;
        }

        //
        // Draw blinking cursor

        if self.cursor_visible {
            let (x, y) = self.formatted.as_ref().index_to_xy(self.prelude_len + self.cursor);
            let cursor_rect = Rect {
                x0: x - ox,
                y0: y - oy,
                w: CURSOR_W,
                h: 20, // TODO
            };
            draw_rect(dst_fb, &cursor_rect, Color::WHITE, false);
        }
    }
}