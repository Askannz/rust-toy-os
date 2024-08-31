use crate::input::{InputEvent, InputState};
use crate::{FbView, FbViewMut, Framebuffer};
use crate::Rect;
use crate::Color;
use crate::drawing::primitives::draw_rect;

use super::{TileRenderer, TileRenderContext, dyn_scrollable_canvas, TileCache};

struct BufferCopyRenderer<'a, F: FbView> {
    src_fb: &'a F,
}

impl<'a, F1: FbView> TileRenderer for BufferCopyRenderer<'a, F1> {

    fn shape(&self) -> (u32, u32) {
        self.src_fb.shape()
    }

    fn render(&self, context: &mut TileRenderContext) {
        context.dst_fb.copy_from_fb(
            &self.src_fb.subregion(&context.src_rect),
            (0, 0),
            false
        );
    }
}

pub fn scrollable_canvas<F1: FbView, F2: FbViewMut>(
    dst_fb: &mut F2,
    dst_rect: &Rect,
    src_fb: &F1,
    offsets: &mut (i64, i64),
    input_state: &InputState,
    dragging: &mut bool,
) {

    let renderer = BufferCopyRenderer { src_fb };

    dyn_scrollable_canvas(
        &mut TileCache::new(),
        dst_fb,
        dst_rect,
        &renderer,
        offsets,
        input_state,
        dragging,
    )
}

pub fn set_autoscroll(
    dst_rect: &Rect,
    max_h: u32,
    offsets: &mut (i64, i64),
) {
    let (_scroll_x0, scroll_y0) = offsets;
    *scroll_y0 = (max_h - dst_rect.h - 1).into();
}

