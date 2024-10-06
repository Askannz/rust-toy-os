use crate::drawing::primitives::draw_rect;
use crate::input::InputEvent;
use crate::Color;
use crate::Rect;
use crate::{BorrowedMutPixels, FbViewMut, Framebuffer};

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
    fn render(&self, context: &mut TileRenderContext);
}

pub struct TileRenderContext<'a> {
    pub dst_fb: &'a mut Framebuffer<BorrowedMutPixels<'a>>,
    pub src_rect: &'a Rect,
    pub tile_cache: &'a mut TileCache,
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

        renderer.render(&mut TileRenderContext {
            dst_fb: &mut dst_fb.subregion_mut(dst_rect),
            src_rect,
            tile_cache,
        });

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
