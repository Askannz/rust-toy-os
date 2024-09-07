
use crate::content::UuidProvider;

use crate::input::InputEvent;
use crate::{Framebuffer, FbViewMut, BorrowedMutPixels};
use crate::Rect;
use crate::Color;
use crate::drawing::primitives::draw_rect;

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

impl<'a, F: FbViewMut, P: UuidProvider> UiContext<'a, F, P> {


pub fn dyn_scrollable_canvas<T: TileRenderer>(
    &mut self,
    dst_rect: &Rect,
    renderer: &T,
    offsets: &mut (i64, i64),
    dragging: &mut bool,
) {

    let UiContext { fb: dst_fb, tile_cache, input_state, .. } = self;

    let (src_max_w, src_max_h) = renderer.shape();
    let (scroll_x0, scroll_y0) = offsets;

    let x_scroll_enabled = false;
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

    //
    // Vertical scrollbar

    if y_scroll_enabled {

        if *dragging {
            *scroll_y0 += (src_max_h as i64) * input_state.pointer.delta_y / (dst_rect.h as i64);
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
            x0: (dst_rect.w - SBAR_OUTER_W + (SBAR_OUTER_W - SBAR_INNER_W) / 2) as i64 + dst_rect.x0,
            y0: (dst_rect.h as i64) * (*scroll_y0) / (src_max_h as i64) + dst_rect.y0,
            w: SBAR_INNER_W,
            h: dst_rect.h * dst_rect.h / src_max_h,
        };

        let p_state = &input_state.pointer;

        let sbar_hover = sbar_inner_rect.check_contains_point(p_state.x, p_state.y);

        let sbar_color = {
            if *dragging {
                SBAR_INNER_DRAGGING_COLOR
            } else if sbar_hover {
                SBAR_INNER_HOVER_COLOR
            } else {
                SBAR_INNER_IDLE_COLOR
            }
        };

        draw_rect(*dst_fb, &sbar_outer_rect, SBAR_OUTER_COLOR);
        draw_rect(*dst_fb, &sbar_inner_rect, sbar_color);

        if p_state.left_clicked {
            if p_state.left_click_trigger && sbar_hover {
                *dragging = true;
            }
        } else {
            *dragging = false;
        }
    }

}
}
