use alloc::vec::Vec;

use crate::drawing::primitives::draw_rect;
use crate::input::InputEvent;
use crate::content::{ContentId, TrackedContent};
use crate::Color;
use crate::Rect;
use crate::{BorrowedMutPixels, FbViewMut, FbView, Framebuffer};

use crate::uitk::{TileCache, UiContext};

const SCROLL_SPEED: u32 = 10;
const SBAR_OUTER_W: u32 = 16;
const SBAR_INNER_W: u32 = 12;
const SBAR_OUTER_COLOR: Color = Color::BLACK;
const SBAR_INNER_IDLE_COLOR: Color = Color::RED;
const SBAR_INNER_HOVER_COLOR: Color = Color::YELLOW;
const SBAR_INNER_DRAGGING_COLOR: Color = Color::AQUA;

pub trait TileRenderer {
    fn shape(&self) -> (u32, u32);
    fn content_id(&self, src_rect: &Rect) -> ContentId;
    fn render(&self, context: &mut TileRenderContext);
}

pub struct TileRenderContext<'a> {
    pub dst_fb: &'a mut Framebuffer<BorrowedMutPixels<'a>>,
    pub src_rect: &'a Rect,
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

        let src_rect = &Rect {
            x0: *scroll_x0,
            y0: *scroll_y0,
            w: dst_rect.w,
            h: dst_rect.h,
        };

        draw_tile(
            renderer,
            &mut TileRenderContext {
                dst_fb: &mut dst_fb.subregion_mut(dst_rect),
                src_rect,
            },
            tile_cache,
        );

        let p_state = &input_state.pointer;
        let (x_dragging, y_dragging) = dragging;

        let get_sbar_color = |dragging, hover| {
            if dragging {
                SBAR_INNER_DRAGGING_COLOR
            } else if hover {
                SBAR_INNER_HOVER_COLOR
            } else {
                SBAR_INNER_IDLE_COLOR
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

            draw_rect(*dst_fb, &sbar_outer_rect, SBAR_OUTER_COLOR, false);
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

            draw_rect(*dst_fb, &sbar_outer_rect, SBAR_OUTER_COLOR, false);
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


fn draw_tile<T: TileRenderer>(renderer: &T, current_tile_context: &mut TileRenderContext, tile_cache: &mut TileCache,) {

    let TileRenderContext {
        dst_fb,
        src_rect,
    } = current_tile_context;


    let src_shape = renderer.shape();
    let tile_shape = (src_rect.w, src_rect.h);

    let tiles_rects = get_tiles(src_shape, tile_shape);

    let regions = select_tile_regions(&tiles_rects, src_rect);

    //log::debug!("{} tiles in cache", tile_cache.tiles.len());

    for tile_region in regions.iter() {

        let tile_content_id = renderer.content_id(&tile_region.tile_rect);

        let tile_fb = tile_cache.tiles.entry(tile_content_id).or_insert_with(|| {
            let mut tile_fb =
                Framebuffer::new_owned(tile_region.tile_rect.w, tile_region.tile_rect.h);

            renderer.render(&mut TileRenderContext {
                dst_fb: &mut tile_fb.subregion_mut(&tile_fb.shape_as_rect()),
                src_rect: &tile_region.tile_rect,
            });

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

        let (dst_x0, dst_y0) = (reg_x0 - src_rect.x0, reg_y0 - src_rect.y0);

        dst_fb.copy_from_fb(&tile_fb.subregion(&tile_src_rect), (dst_x0, dst_y0), false);
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

fn get_tiles(src_shape: (u32, u32), tile_shape: (u32, u32)) -> Vec<Rect> {

    let (cw, ch) = src_shape;
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
