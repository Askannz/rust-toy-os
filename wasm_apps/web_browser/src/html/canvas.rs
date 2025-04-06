use super::layout::{LayoutNode, NodeData};
use super::render_list::RenderItem;
use super::render::render_html2;
use applib::content::{ContentId, TrackedContent};
use applib::drawing::primitives::{draw_rect, draw_rect_outline};
use applib::uitk::{self, TileRenderer, UiContext};
use applib::Color;
use applib::Rect;
use applib::{FbView, FbViewMut, Framebuffer};

pub fn html_canvas<'a, F: FbViewMut>(
    uitk_context: &mut UiContext<'a, F>,
    render_list: &'a TrackedContent<Vec<RenderItem>>,
    dst_rect: &Rect,
    offsets: &mut (i64, i64),
    dragging: &mut (bool, bool),
) -> Option<&'a str> {
    uitk_context.dynamic_canvas(dst_rect, &HtmlRenderer { render_list }, offsets, dragging);

    let UiContext {
        fb, input_state, ..
    } = uitk_context;

    let (ox, oy) = *offsets;
    let p = &input_state.pointer;
    let vr = dst_rect;

    // match get_hovered_link(p.x - vr.x0 + ox, p.y - vr.y0 + oy, layout.as_ref()) {
    //     Some(link_data) => {
    //         let rect = Rect {
    //             x0: link_data.rect.x0 + vr.x0 - ox,
    //             y0: link_data.rect.y0 + vr.y0 - oy,
    //             w: link_data.rect.w,
    //             h: link_data.rect.h,
    //         };
    //         draw_rect(*fb, &rect, Color::rgba(0, 0, 255, 128), true);
    //         Some(&link_data.url)
    //     }
    //     None => None,
    // }

    None
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
    render_list: &'a TrackedContent<Vec<RenderItem>>,
}

impl<'a> uitk::TileRenderer for HtmlRenderer<'a> {
    fn shape(&self) -> (u32, u32) {

        let Rect { w, h, .. } = get_render_rect(self.render_list.as_ref());
        (w, h)
    }

    fn tile_shape(&self) -> (u32, u32) {
        let Rect { w, .. } = get_render_rect(self.render_list.as_ref());
        (
            u32::max(w, 300),
            300
        )
    }

    fn content_id(&self, tile_rect: &Rect) -> ContentId {

        let layout_rect = get_render_rect(self.render_list.as_ref());

        if tile_rect.intersection(&layout_rect).is_none() {
            ContentId::from_hash(&(tile_rect.w, tile_rect.h))
        } else {
            ContentId::from_hash(&(
                tile_rect,
                self.render_list.get_id()
            ))
        }
    }

    fn render<F: FbViewMut>(&self, dst_fb: &mut F, tile_rect: &Rect) {
        //log::debug!("Rendering HTML tile");
        dst_fb.fill(Color::WHITE);

        // DEBUG
        draw_rect_outline(dst_fb, tile_rect, Color::GREEN, false, 1);

        render_html2(dst_fb, self.render_list.as_ref(), tile_rect);
    }
}


fn get_render_rect(render_list: &[RenderItem]) -> Rect {
    let root = render_list.as_ref().last().expect("Empty render list");
    root.get_rect()
}
