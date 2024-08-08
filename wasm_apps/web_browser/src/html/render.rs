use applib::{Color, Rect, Framebuffer};
use applib::drawing::primitives::{draw_rect, blend_rect};
use applib::drawing::text::draw_str;

use super::layout::{LayoutNode, NodeData};

pub fn render_html(fb: &mut Framebuffer, layout: &LayoutNode) {

    let first_node = layout;
    draw_node(fb, first_node)
}


fn draw_node(fb: &mut Framebuffer, node: &LayoutNode) {

    let rect = &node.rect;

    if fb.w as i64 <= rect.x0 || fb.h as i64 <= rect.y0 {
        return;
    }

    match &node.data {
        NodeData::Text { text, color, font, .. } => {
            draw_str(fb, text, rect.x0, rect.y0, font, *color, None);
        },
        NodeData::Image => (),
        NodeData::Container { children, bg_color, .. } => {

            if let &Some(bg_color) = bg_color {
                draw_rect(fb, &rect, bg_color);
            }

            for child in children.iter() {
                draw_node(fb, child);
            }
        }
    }
}
