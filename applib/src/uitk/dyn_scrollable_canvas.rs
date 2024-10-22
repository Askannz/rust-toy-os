use alloc::vec::Vec;

use crate::drawing::primitives::draw_rect;
use crate::input::InputEvent;
use crate::content::{ContentId, TrackedContent};
use crate::Color;
use crate::Rect;
use crate::{BorrowedMutPixels, FbViewMut, FbView, Framebuffer};

use crate::uitk::{CachedTile, TileCache, UiContext};

const SCROLL_SPEED: u32 = 10;
const SBAR_OUTER_W: u32 = 16;
const SBAR_INNER_W: u32 = 12;

pub trait TileRenderer {
    fn shape(&self) -> (u32, u32);
    fn tile_shape(&self) -> (u32, u32);
    fn content_id(&self, viewport_rect: &Rect) -> ContentId;
    fn render<F: FbViewMut>(&self, dst_fb: &mut F, viewport_rect: &Rect);
}

impl<'a, F: FbViewMut> UiContext<'a, F> {
    pub fn dyn_scrollable_canvas<T: TileRenderer>(
        &mut self,
        dst_rect: &Rect,
        renderer: &T,
        offsets: &mut (i64, i64),
        dragging: &mut (bool, bool),
    ) {
        let UiContext {
            fb: dst_fb,
            tile_cache,
            input_state,
            stylesheet,
            ..
        } = self;

        let (src_max_w, src_max_h) = renderer.shape();
        let (scroll_x0, scroll_y0) = offsets;

        let x_scroll_enabled = src_max_w > dst_rect.w;
        let y_scroll_enabled = src_max_h > dst_rect.h;

        if x_scroll_enabled {
            *scroll_x0 = i64::max(0, *scroll_x0);
            *scroll_x0 = i64::min((src_max_w - dst_rect.w - 1).into(), *scroll_x0);
        } else {
            *scroll_x0 = 0;
        }

        if y_scroll_enabled {
            *scroll_y0 = i64::max(0, *scroll_y0);
            *scroll_y0 = i64::min((src_max_h - dst_rect.h - 1).into(), *scroll_y0);
        } else {
            *scroll_y0 = 0;
        }

        let viewport_rect = &Rect {
            x0: *scroll_x0,
            y0: *scroll_y0,
            w: dst_rect.w,
            h: dst_rect.h,
        };

        let mut dst_subregion = dst_fb.subregion_mut(dst_rect);

        draw_tiles(
            renderer,
            &mut dst_subregion,
            viewport_rect,
            self.time,
            tile_cache,
        );

        let p_state = &input_state.pointer;
        let (x_dragging, y_dragging) = dragging;

        let get_sbar_color = |dragging, hover| {
            if dragging {
                stylesheet.colors.selected_overlay
            } else if hover {
                stylesheet.colors.hover_overlay
            } else {
                stylesheet.colors.accent
            }
        };

        //
        // Vertical scrollbar

        if y_scroll_enabled {
            if *y_dragging {
                *scroll_y0 +=
                    (src_max_h as i64) * input_state.pointer.delta_y / (dst_rect.h as i64);
            } else {
                for event in input_state.events {
                    if let Some(InputEvent::Scroll { delta }) = event {
                        *scroll_y0 -= delta * (SCROLL_SPEED as i64);
                    }
                }
            }

            let sbar_outer_rect = Rect {
                x0: (dst_rect.w - SBAR_OUTER_W).into(),
                y0: dst_rect.y0,
                w: SBAR_OUTER_W,
                h: dst_rect.h,
            };

            let sbar_inner_rect = Rect {
                x0: (dst_rect.w - SBAR_OUTER_W + (SBAR_OUTER_W - SBAR_INNER_W) / 2) as i64
                    + dst_rect.x0,
                y0: (dst_rect.h as i64) * (*scroll_y0) / (src_max_h as i64) + dst_rect.y0,
                w: SBAR_INNER_W,
                h: sbar_outer_rect.h * dst_rect.h / src_max_h,
            };

            let sbar_hover = sbar_inner_rect.check_contains_point(p_state.x, p_state.y);

            let sbar_color = get_sbar_color(*y_dragging, sbar_hover);

            draw_rect(*dst_fb, &sbar_inner_rect, sbar_color, false);

            if p_state.left_clicked {
                if p_state.left_click_trigger && sbar_hover {
                    *y_dragging = true;
                }
            } else {
                *y_dragging = false;
            }
        }

        //
        // Horizontal scrollbar

        if x_scroll_enabled {
            if *x_dragging {
                *scroll_x0 +=
                    (src_max_w as i64) * input_state.pointer.delta_x / (dst_rect.w as i64);
            }

            let sbar_outer_rect = Rect {
                x0: dst_rect.x0,
                y0: dst_rect.y0 + (dst_rect.h - SBAR_OUTER_W) as i64,
                w: dst_rect.w,
                h: SBAR_OUTER_W,
            };

            let sbar_inner_rect = Rect {
                y0: (dst_rect.h - SBAR_OUTER_W + (SBAR_OUTER_W - SBAR_INNER_W) / 2) as i64
                    + dst_rect.y0,
                x0: (dst_rect.w as i64) * (*scroll_x0) / (src_max_w as i64) + dst_rect.x0,
                h: SBAR_INNER_W,
                w: sbar_outer_rect.w * dst_rect.w / src_max_w,
            };

            let sbar_hover = sbar_inner_rect.check_contains_point(p_state.x, p_state.y);

            let sbar_color = get_sbar_color(*x_dragging, sbar_hover);

            draw_rect(*dst_fb, &sbar_inner_rect, sbar_color, false);

            if p_state.left_clicked {
                if p_state.left_click_trigger && sbar_hover {
                    *x_dragging = true;
                }
            } else {
                *x_dragging = false;
            }
        }
    }
}


fn draw_tiles<F: FbViewMut, T: TileRenderer>(
    renderer: &T,
    dst_fb: &mut F,
    viewport_rect: &Rect,  // Shape of current viewport in the src canvas
    time: f64,
    tile_cache: &mut TileCache
) {

    let src_canvas_shape = renderer.shape();  // Shape of the full src canvas
    let tile_shape = renderer.tile_shape(); // Shape of individual tiles

    let (vw, vh) = (viewport_rect.w, viewport_rect.h);
    let tiles_rects = get_tiles(src_canvas_shape, (vw, vh), tile_shape);

    //log::debug!("{} tiles in cache", tile_cache.tiles.len());

    let regions = select_tile_regions(&tiles_rects, viewport_rect);

    for tile_region in regions.iter() {

        let tile_content_id = renderer.content_id(&tile_region.tile_rect);

        let tile_fb = tile_cache.fetch_or_create(tile_content_id, time, || {
            let mut tile_fb =
                Framebuffer::new_owned(tile_region.tile_rect.w, tile_region.tile_rect.h);

            let mut dst_fb = tile_fb.subregion_mut(&tile_fb.shape_as_rect());
            renderer.render(&mut dst_fb, &tile_region.tile_rect); 

            draw_tile_border(&mut tile_fb);

            tile_fb
        });

        let Rect {
            x0: tile_x0,
            y0: tile_y0,
            ..
        } = tile_region.tile_rect;
        let Rect {
            x0: reg_x0,
            y0: reg_y0,
            w: reg_w,
            h: reg_h,
        } = tile_region.region_rect;

        let tile_src_rect = Rect {
            x0: reg_x0 - tile_x0,
            y0: reg_y0 - tile_y0,
            w: reg_w,
            h: reg_h,
        };

        let (dst_x0, dst_y0) = (reg_x0 - viewport_rect.x0, reg_y0 - viewport_rect.y0);

        dst_fb.copy_from_fb(&tile_fb.subregion(&tile_src_rect), (dst_x0, dst_y0), false);
    }
}


fn select_tile_regions(tiles_rects: &Vec<Rect>, viewport_rect: &Rect) -> Vec<TileRegion> {
    let mut regions = Vec::new();
    for tile_rect in tiles_rects {
        match tile_rect.intersection(viewport_rect) {
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

fn get_tiles(src_canvas_shape: (u32, u32), viewport_shape: (u32, u32), tile_shape: (u32, u32)) -> Vec<Rect> {

    let (cw, ch) = src_canvas_shape;
    let (vw, vh) = viewport_shape;
    let (tile_w, tile_h) = tile_shape;

    let cov_w = u32::max(cw, vw);
    let cov_h = u32::max(ch, vh);

    let n_tiles_x = if cov_w % tile_w == 0 {
        cov_w / tile_w
    } else {
        cov_w / tile_w + 1
    };

    let n_tiles_y = if cov_h % tile_h == 0 {
        cov_h / tile_h
    } else {
        cov_h / tile_h + 1
    };

    let mut tiles_rects = Vec::new();
    for ix in 0..n_tiles_x {
        for iy in 0..n_tiles_y {

            let x1 = ix * tile_w;
            let x2 = (ix + 1) * tile_w - 1;
            let y1 = iy * tile_h;
            let y2 = (iy + 1) * tile_h - 1;

            tiles_rects.push(Rect::from_xyxy([x1.into(), y1.into(), x2.into(), y2.into()]))
        }
    }

    tiles_rects
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
