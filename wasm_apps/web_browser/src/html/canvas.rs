use applib::input::{InputEvent, InputState, PointerState};
use applib::drawing::primitives::{draw_rect, blend_rect};
use applib::Framebuffer;
use applib::Rect;
use applib::Color;
use super::layout::{LayoutNode, NodeData};

pub fn html_canvas<'a>(
    fb: &mut Framebuffer,
    layout: &'a LayoutNode,
    view_rect: &Rect,
    offsets: (i64, i64),
    input_state: &InputState
) -> Option<&'a str> {

    let (ox, oy) = offsets;
    let p = &input_state.pointer;
    let vr = view_rect;

    if !view_rect.check_contains_point(p.x, p.y) {
        return None;
    }

    match get_hovered_link(p.x - vr.x0 + ox, p.y - vr.y0 + oy, layout) {
        Some(link_data) => {
            let draw_rect = Rect {
                x0: link_data.rect.x0 + vr.x0 - ox,
                y0: link_data.rect.y0 + vr.y0 - oy,
                w: link_data.rect.w,
                h: link_data.rect.h,
            };
            blend_rect(fb, &draw_rect, Color::rgba(0, 0, 255, 128));
            Some(&link_data.url)
        }
        None => None,
    }
}

struct LinkData<'a> {
    rect: Rect,
    url: &'a str,
}

fn get_hovered_link(x: i64, y: i64, node: &LayoutNode) -> Option<LinkData> {

    let rect = &node.rect;

    match &node.data {
        NodeData::Container { children, url, .. } => match rect.check_contains_point(x, y) {
            true => match url {
                Some(url) => Some(LinkData {
                    rect: rect.clone(),
                    url: url.as_str(),
                }),
                None => children.iter().find_map(|c| get_hovered_link(x, y, c))
            },
            false => None
        },
        _ => None,
    }

}
