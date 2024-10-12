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

    fn content_id(&self, src_rect: &Rect) -> ContentId {
        ContentId::from_hash((
            src_rect,
            self.layout.get_id()
        ))
    }

    fn render(&self, context: &mut uitk::TileRenderContext) {
        let uitk::TileRenderContext {
            dst_fb,
            src_rect,
        } = context;

        render_html(*dst_fb, self.layout.as_ref(), src_rect);
    }
}

fn render_gradient<F: FbViewMut>(tile_fb: &mut F, canvas_shape: (u32, u32), tile_pos: (i64, i64)) {
    let (canvas_w, canvas_h) = canvas_shape;
    let (tile_x0, tile_y0) = tile_pos;
    let (tile_w, tile_h) = tile_fb.shape();

    for dx in 0..tile_w as i64 {
        for dy in 0..tile_h as i64 {
            let x = tile_x0 + dx;
            let y = tile_y0 + dy;

            let r = (255 * x / canvas_w as i64) as u8;
            let g = (255 * y / canvas_h as i64) as u8;
            let b = 0;

            tile_fb.set_pixel(dx, dy, Color::rgb(r, g, b));
        }
    }
}

fn draw_tile_border<F: FbViewMut>(tile_fb: &mut F) {
    const THICKNESS: u32 = 1;
    const COLOR: Color = Color::RED;

    let (w, h) = tile_fb.shape();

    let r_top = Rect {
        x0: 0,
        y0: 0,
        w,
        h: THICKNESS,
    };
    let r_left = Rect {
        x0: 0,
        y0: 0,
        w: THICKNESS,
        h,
    };
    let r_bottom = Rect {
        x0: 0,
        y0: (h - THICKNESS).into(),
        w,
        h: THICKNESS,
    };
    let r_right = Rect {
        x0: (w - THICKNESS).into(),
        y0: 0,
        w: THICKNESS,
        h,
    };

    draw_rect(tile_fb, &r_top, COLOR, false);
    draw_rect(tile_fb, &r_left, COLOR, false);
    draw_rect(tile_fb, &r_bottom, COLOR, false);
    draw_rect(tile_fb, &r_right, COLOR, false);
}

impl<'a> HtmlRenderer<'a> {
    fn get_tiles(&self, tile_shape: (u32, u32)) -> Vec<Rect> {
        let Rect { w: cw, h: ch, .. } = self.layout.as_ref().rect;
        let (tile_w, tile_h) = tile_shape;

        let cw: i64 = cw.into();
        let ch: i64 = ch.into();
        let tile_w: i64 = tile_w.into();
        let tile_h: i64 = tile_h.into();

        let mut tile_bounds_x = Vec::new();
        let mut x = 0;
        while x < cw {
            let new_x = i64::min(x + tile_w, cw);
            tile_bounds_x.push((x, new_x));
            x = new_x;
        }

        let mut tile_bounds_y = Vec::new();
        let mut y = 0;
        while y < ch {
            let new_y = i64::min(y + tile_h, ch);
            tile_bounds_y.push((y, new_y));
            y = new_y;
        }

        let mut tiles_rects = Vec::new();
        for (x0, x1) in tile_bounds_x.iter() {
            for (y0, y1) in tile_bounds_y.iter() {
                tiles_rects.push(Rect::from_xyxy([*x0, *y0, *x1 - 1, *y1 - 1]))
            }
        }

        tiles_rects
    }
}

fn select_tile_regions(tiles_rects: &Vec<Rect>, src_rect: &Rect) -> Vec<TileRegion> {
    let mut regions = Vec::new();
    for tile_rect in tiles_rects {
        match tile_rect.intersection(src_rect) {
            None => (),
            Some(region_rect) => regions.push(TileRegion {
                tile_rect: tile_rect.clone(),
                region_rect: region_rect.clone(),
            }),
        }
    }

    regions
}

#[derive(Debug)]
struct TileRegion {
    tile_rect: Rect,
    region_rect: Rect,
}
