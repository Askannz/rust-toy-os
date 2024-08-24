use applib::input::{InputEvent, InputState, PointerState};
use applib::drawing::primitives::{draw_rect, blend_rect};
use applib::Framebuffer;
use applib::Rect;
use applib::Color;
use applib::uitk;
use super::layout::{LayoutNode, NodeData};
use super::render::render_html;

pub fn html_canvas<'a>(
    fb: &mut Framebuffer,
    layout: &'a LayoutNode,
    dst_rect: &Rect,
    offsets: &mut (i64, i64),
    dragging: &mut bool,
    input_state: &InputState
) -> Option<&'a str> {

    let (ox, oy) = *offsets;
    let p = &input_state.pointer;
    let vr = dst_rect;

    uitk::dyn_scrollable_canvas(
        fb,
        dst_rect,
        &HtmlRenderer { layout },
        offsets,
        input_state,
        dragging,
    );

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

struct HtmlRenderer<'a> {
    layout: &'a LayoutNode,
}

impl<'a> uitk::TileRenderer for HtmlRenderer<'a> {

    fn shape(&self) -> (u32, u32) {
       let Rect { w, h, .. } = self.layout.rect;
       (w, h)
    }

    fn render(&self, context: &mut uitk::TileRenderContext) {

        let uitk::TileRenderContext { dst_fb, dst_rect, src_rect, .. } = context;

        render_html(dst_fb, dst_rect, &self.layout, src_rect);

    }
}
