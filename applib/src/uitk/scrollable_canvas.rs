use crate::uitk::UiContext;
use crate::Rect;
use crate::{FbView, FbViewMut};
use crate::content::{TrackedContent, ContentId};

use super::{TileRenderer};

struct BufferCopyRenderer<'a, F: FbView> {
    src_fb: &'a TrackedContent<F>,
}

impl<'a, F1: FbView> TileRenderer for BufferCopyRenderer<'a, F1> {
    fn shape(&self) -> (u32, u32) {
        self.src_fb.as_ref().shape()
    }

    fn max_tile_shape(&self, viewport_rect: &Rect) -> (u32, u32) {
        (viewport_rect.w, viewport_rect.h)
    }

    fn content_id(&self, viewport_rect: &Rect) -> ContentId {
        ContentId::from_hash((
            self.src_fb.get_id(),
            viewport_rect,
        ))
    }

    fn render<F: FbViewMut>(&self, dst_fb: &mut F, viewport_rect: &Rect) {
        let src_fb = self.src_fb.as_ref().subregion(viewport_rect);
        dst_fb.copy_from_fb(&src_fb, (0, 0), false);
    }
}

impl<'a, F: FbViewMut> UiContext<'a, F> {
    pub fn scrollable_canvas<F1: FbView>(
        &mut self,
        dst_rect: &Rect,
        src_fb: &TrackedContent<F1>,
        offsets: &mut (i64, i64),
        dragging: &mut (bool, bool),
    ) {
        let renderer = BufferCopyRenderer { src_fb };

        self.dyn_scrollable_canvas(dst_rect, &renderer, offsets, dragging)
    }
}

pub fn set_autoscroll(dst_rect: &Rect, max_h: u32, offsets: &mut (i64, i64)) {
    let (_scroll_x0, scroll_y0) = offsets;
    *scroll_y0 = (max_h - dst_rect.h - 1).into();
}
