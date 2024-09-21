use crate::uitk::UiContext;
use crate::Rect;
use crate::{FbView, FbViewMut};

use super::{TileRenderContext, TileRenderer};

struct BufferCopyRenderer<'a, F: FbView> {
    src_fb: &'a F,
}

impl<'a, F1: FbView> TileRenderer for BufferCopyRenderer<'a, F1> {
    fn shape(&self) -> (u32, u32) {
        self.src_fb.shape()
    }

    fn render(&self, context: &mut TileRenderContext) {
        context
            .dst_fb
            .copy_from_fb(&self.src_fb.subregion(&context.src_rect), (0, 0), false);
    }
}

impl<'a, F: FbViewMut> UiContext<'a, F> {
    pub fn scrollable_canvas<F1: FbView>(
        &mut self,
        dst_rect: &Rect,
        src_fb: &F1,
        offsets: &mut (i64, i64),
        dragging: &mut bool,
    ) {
        let renderer = BufferCopyRenderer { src_fb };

        self.dyn_scrollable_canvas(dst_rect, &renderer, offsets, dragging)
    }
}

pub fn set_autoscroll(dst_rect: &Rect, max_h: u32, offsets: &mut (i64, i64)) {
    let (_scroll_x0, scroll_y0) = offsets;
    *scroll_y0 = (max_h - dst_rect.h - 1).into();
}
