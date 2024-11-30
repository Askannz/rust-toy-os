use alloc::vec::Vec;
use alloc::string::String;

use crate::drawing::primitives::draw_rect;
use crate::drawing::text::{draw_rich_slice, format_rich_lines, Font, FormattedRichText, RichText};
use crate::input::InputEvent;
use crate::content::{ContentId, TrackedContent};
use crate::{Color, StyleSheet};
use crate::Rect;
use crate::{BorrowedMutPixels, FbViewMut, FbView, Framebuffer};

use crate::uitk::{CachedTile, TileCache, TileRenderer, UiContext};


impl<'a, F: FbViewMut> UiContext<'a, F> {

    pub fn scrollable_text<T: FormattableText>(
        &mut self,
        dst_rect: &Rect,
        text: &T,
        offsets: &mut (i64, i64),
        dragging: &mut (bool, bool),
        autoscroll: bool,
    ) {

        // Only used if text is not already a RichText
        let font = self.font_family.get_default();
        let color = self.stylesheet.colors.text;

        let rich_text = text.to_rich_text(color, font);

        let formatted = {
            let formatted = format_rich_lines(rich_text.as_ref(), dst_rect.w);
            let content_id = ContentId::from_hash((
                rich_text.get_id(),
                dst_rect.w,
            ));
            TrackedContent::new_with_id(formatted, content_id)
        };

        let renderer = TextRenderer { formatted };

        if autoscroll {
            let (_, scroll_y0) = offsets;
            let (_, max_h) = renderer.shape();
            *scroll_y0 = (max_h - dst_rect.h - 1).into();
        }

        self.dynamic_canvas(
            dst_rect,
            &renderer,
            offsets,
            dragging,
        );

        
    }

}

pub trait FormattableText {
    fn to_rich_text(&self, color: Color, font: &'static Font) -> TrackedContent<RichText>;
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
}

impl FormattableText for TrackedContent<RichText> {
    fn to_rich_text(&self, _color: Color, _font: &'static Font) -> TrackedContent<RichText> {
        let content_id = self.get_id();
        let rich_text = self.as_ref().clone();
        TrackedContent::new_with_id(rich_text, content_id)
    }
}

struct TextRenderer {
    formatted: TrackedContent<FormattedRichText>,
}


impl TileRenderer for TextRenderer {
    fn shape(&self) -> (u32, u32) {
        let FormattedRichText { w, h, .. } = *self.formatted.as_ref();
        (w, h)
    }

    fn tile_shape(&self) -> (u32, u32) {
        let FormattedRichText { w, .. } = *self.formatted.as_ref();
        (
            u32::max(w, 200),
            200
        )
    }

    fn content_id(&self, tile_rect: &Rect) -> ContentId {

        let FormattedRichText { w, h, .. } = *self.formatted.as_ref();
        let text_rect = Rect { x0: 0, y0: 0, w, h};

        if tile_rect.intersection(&text_rect).is_none() {
            ContentId::from_hash((tile_rect.w, tile_rect.h))
        } else {
            ContentId::from_hash((
                tile_rect,
                self.formatted.get_id()
            ))
        }
    }

    fn render<F: FbViewMut>(&self, dst_fb: &mut F, tile_rect: &Rect) {

        let Rect { x0: ox, y0: oy, .. } = *tile_rect;

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
    }
}