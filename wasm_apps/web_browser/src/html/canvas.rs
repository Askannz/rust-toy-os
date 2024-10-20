use super::layout::{LayoutNode, NodeData};
use super::render::render_html;
use applib::content::{ContentId, TrackedContent};
use applib::drawing::primitives::{blend_rect, draw_rect};
use applib::uitk::{self, TileRenderer, UiContext};
use applib::Color;
use applib::Rect;
use applib::{FbView, FbViewMut, Framebuffer};

pub fn html_canvas<'a, F: FbViewMut>(
    uitk_context: &mut UiContext<'a, F>,
    layout: &'a TrackedContent<LayoutNode>,
    dst_rect: &Rect,
    offsets: &mut (i64, i64),
    dragging: &mut (bool, bool),
) -> Option<&'a str> {
    uitk_context.dyn_scrollable_canvas(dst_rect, &HtmlRenderer { layout }, offsets, dragging);

    let UiContext {
        fb, input_state, ..
    } = uitk_context;

    let (ox, oy) = *offsets;
    let p = &input_state.pointer;
    let vr = dst_rect;

    match get_hovered_link(p.x - vr.x0 + ox, p.y - vr.y0 + oy, layout.as_ref()) {
        Some(link_data) => {
            let draw_rect = Rect {
                x0: link_data.rect.x0 + vr.x0 - ox,
                y0: link_data.rect.y0 + vr.y0 - oy,
                w: link_data.rect.w,
                h: link_data.rect.h,
            };
            blend_rect(*fb, &draw_rect, Color::rgba(0, 0, 255, 128));
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
                None => children.iter().find_map(|c| get_hovered_link(x, y, c)),
            },
            false => None,
        },
        _ => None,
    }
}

struct HtmlRenderer<'a> {
    layout: &'a TrackedContent<LayoutNode>,
}

impl<'a> uitk::TileRenderer for HtmlRenderer<'a> {
    fn shape(&self) -> (u32, u32) {
        let Rect { w, h, .. } = self.layout.as_ref().rect;
        (w, h)
    }

    fn max_tile_shape(&self, _viewport_rect: &Rect) -> (u32, u32) {
        let Rect { w, .. } = self.layout.as_ref().rect;
        (w, 300)
    }

    fn content_id(&self, src_rect: &Rect) -> ContentId {
        ContentId::from_hash((
            src_rect,
            self.layout.get_id()
        ))
    }

    fn render<F: FbViewMut>(&self, dst_fb: &mut F, src_rect: &Rect) {
        render_html(dst_fb, self.layout.as_ref(), src_rect);
    }
}
