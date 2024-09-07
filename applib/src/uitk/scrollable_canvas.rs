use crate::content::UuidProvider;
use crate::input::{InputEvent, InputState};
use crate::{FbView, FbViewMut, Framebuffer};
use crate::Rect;
use crate::Color;
use crate::drawing::primitives::draw_rect;
use crate::uitk::{UiStore, UiContext};

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

impl<'a, F: FbViewMut, P: UuidProvider> UiContext<'a, F, P> {

pub fn scrollable_canvas<F1: FbView>(
    &mut self,
    dst_rect: &Rect,
    src_fb: &F1,
    offsets: &mut (i64, i64),
    dragging: &mut bool,
) {

    let renderer = BufferCopyRenderer { src_fb };

    self.dyn_scrollable_canvas(
        dst_rect,
        &renderer,
        offsets,
        dragging,
    )
}
}

pub fn set_autoscroll(
    dst_rect: &Rect,
    max_h: u32,
    offsets: &mut (i64, i64),
) {
    let (_scroll_x0, scroll_y0) = offsets;
    *scroll_y0 = (max_h - dst_rect.h - 1).into();
}

