use crate::input::{InputEvent, InputState};
use crate::Framebuffer;
use crate::Rect;
use crate::Color;
use crate::drawing::primitives::draw_rect;

use super::{TileRenderer, TileRenderContext, dyn_scrollable_canvas};

struct BufferCopyRenderer<'a> {
    src_fb: &'a Framebuffer<'a>,
}

impl<'a> TileRenderer for BufferCopyRenderer<'a> {

    fn shape(&self) -> (u32, u32) {
        (self.src_fb.w, self.src_fb.h)
    }

    fn render(&self, context: &mut TileRenderContext) {
        context.dst_fb.copy_from_fb(
            self.src_fb,
            context.src_rect,
            context.dst_rect,
            false
        );
    }
}

pub fn scrollable_canvas(
    dst_fb: &mut Framebuffer,
    dst_rect: &Rect,
    src_fb: &Framebuffer,
    offsets: &mut (i64, i64),
    input_state: &InputState,
    dragging: &mut bool,
) {

    let renderer = BufferCopyRenderer { src_fb };

    dyn_scrollable_canvas(
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
    src_fb: &Framebuffer,
    offsets: &mut (i64, i64),
) {
    let (_scroll_x0, scroll_y0) = offsets;
    *scroll_y0 = (src_fb.h - dst_rect.h - 1).into();
}

