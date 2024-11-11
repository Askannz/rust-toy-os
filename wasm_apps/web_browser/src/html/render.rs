use applib::drawing::primitives::draw_rect;
use applib::drawing::text::draw_str;
use applib::uitk::render_rich_text;
use applib::{FbViewMut, Rect};

use super::layout::{LayoutNode, NodeData};

pub fn render_html<F: FbViewMut>(dst_fb: &mut F, layout: &LayoutNode, src_rect: &Rect) {
    let first_node = layout;
    draw_node(dst_fb, first_node, src_rect)
}

fn draw_node<F: FbViewMut>(dst_fb: &mut F, node: &LayoutNode, src_rect: &Rect) {
    let Rect { x0: ox, y0: oy, .. } = *src_rect;

    let node_rect = &node.rect;

    let inter_rect = match node_rect.intersection(src_rect) {
        None => return,
        Some(rect) => rect,
    };

    let abs_rect = Rect {
        x0: inter_rect.x0 - ox,
        y0: inter_rect.y0 - oy,
        w: inter_rect.w,
        h: inter_rect.h,
    };

    match &node.data {
        NodeData::Text {
            text, ..
        } => {

            let draw_x0 = node_rect.x0 - ox;
            let draw_y0 = node_rect.y0 - oy;

            let draw_rect = Rect { x0: draw_x0, y0: draw_y0, w: text.w, h: text.h };

            render_rich_text(dst_fb, &draw_rect, text, (0, 0));
        }
        NodeData::Image => (),
        NodeData::Container {
            children, bg_color, ..
        } => {
            if let &Some(bg_color) = bg_color {
                draw_rect(dst_fb, &abs_rect, bg_color, false);
            }

            for child in children.iter() {
                draw_node(dst_fb, child, src_rect);
            }
        }
    }
}
