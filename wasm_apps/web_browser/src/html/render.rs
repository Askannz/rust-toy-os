use applib::{Color, Rect, Framebuffer};
use applib::drawing::primitives::{draw_rect, blend_rect};
use applib::drawing::text::draw_str;

use super::layout::{LayoutNode, NodeData};

pub fn render_html(dst_fb: &mut Framebuffer, dst_rect: &Rect, layout: &LayoutNode, src_rect: &Rect) {

    let first_node = layout;
    draw_node(dst_fb, dst_rect, first_node, src_rect)
}


fn draw_node(dst_fb: &mut Framebuffer, dst_rect: &Rect, node: &LayoutNode, src_rect: &Rect) {

    let Rect { x0: dst_x0, y0: dst_y0, .. } = *dst_rect;
    let Rect { x0: ox, y0: oy, .. } = *src_rect;

    let node_rect = &node.rect;

    let inter_rect = match node_rect.intersection(src_rect) {
        None => return,
        Some(rect) => rect,
    };

    let abs_rect = Rect {
        x0: inter_rect.x0 - ox + dst_x0,
        y0: inter_rect.y0 - oy + dst_y0,
        w: inter_rect.w,
        h: inter_rect.h,
    };

    match &node.data {
        NodeData::Text { text, color, font, .. } => {
            // HACK to hide text lines partially off-screen
            // (can't do partial draws yet)
            if inter_rect.w == node_rect.w && inter_rect.h == node_rect.h {
                draw_str(dst_fb, text, abs_rect.x0, abs_rect.y0, font, *color, None);
            }
        },
        NodeData::Image => (),
        NodeData::Container { children, bg_color, .. } => {

            if let &Some(bg_color) = bg_color {
                draw_rect(dst_fb, &abs_rect, bg_color);
            }

            for child in children.iter() {
                draw_node(dst_fb, dst_rect, child, src_rect);
            }
        }
    }
}
