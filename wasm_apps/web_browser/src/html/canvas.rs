use super::layout::{LayoutNode, NodeData};
use super::render::render_html;
use applib::content::{ContentId, TrackedContent};
use applib::drawing::primitives::{draw_rect};
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
    uitk_context.dynamic_canvas(dst_rect, &HtmlRenderer { layout }, offsets, dragging);

    let UiContext {
        fb, input_state, ..
    } = uitk_context;

    let (ox, oy) = *offsets;
    let p = &input_state.pointer;
    let vr = dst_rect;

    match get_hovered_link(p.x - vr.x0 + ox, p.y - vr.y0 + oy, layout.as_ref()) {
        Some(link_data) => {
            let rect = Rect {
                x0: link_data.rect.x0 + vr.x0 - ox,
                y0: link_data.rect.y0 + vr.y0 - oy,
                w: link_data.rect.w,
                h: link_data.rect.h,
            };
            draw_rect(*fb, &rect, Color::rgba(0, 0, 255, 128), true);
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

    fn tile_shape(&self) -> (u32, u32) {
        let Rect { w, .. } = self.layout.as_ref().rect;
        (
            u32::max(w, 300),
            300
        )
    }

    fn content_id(&self, tile_rect: &Rect) -> ContentId {

        let layout_rect = &self.layout.as_ref().rect;

        if tile_rect.intersection(layout_rect).is_none() {
            ContentId::from_hash((tile_rect.w, tile_rect.h))
        } else {
            ContentId::from_hash((
                tile_rect,
                self.layout.get_id()
            ))
        }
    }

    fn render<F: FbViewMut>(&self, dst_fb: &mut F, tile_rect: &Rect) {
        //log::debug!("Rendering HTML tile");
        render_html(dst_fb, self.layout.as_ref(), tile_rect);
    }
}
