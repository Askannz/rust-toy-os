use crate::input::{InputEvent, InputState};
use crate::Framebuffer;
use crate::Rect;
use crate::Color;
use crate::drawing::primitives::draw_rect;


const SCROLL_SPEED: u32 = 5;
const SBAR_OUTER_W: u32 = 8;
const SBAR_INNER_W: u32 = 4;
const SBAR_OUTER_COLOR: Color = Color::BLACK;
const SBAR_INNER_IDLE_COLOR: Color = Color::RED;
const SBAR_INNER_HOVER_COLOR: Color = Color::YELLOW;
const SBAR_INNER_DRAGGING_COLOR: Color = Color::AQUA;

pub fn scrollable_canvas(
    dst_fb: &mut Framebuffer,
    dst_rect: &Rect,
    src_fb: &Framebuffer,
    src_rect: &mut Rect,
    input_state: &InputState,
    dragging: &mut bool,
) {

    if *dragging {
        src_rect.y0 += (src_fb.h as i64) * input_state.pointer.delta_y / (dst_rect.h as i64);
    } else {
        for event in input_state.events {
            if let Some(InputEvent::Scroll { delta }) = event {
                src_rect.y0 -= delta * (SCROLL_SPEED as i64);
            }
        }
    }

    // TODO: what if source buffer is smaller than dest rect?
    src_rect.x0 = i64::max(0, src_rect.x0);
    src_rect.y0 = i64::max(0, src_rect.y0);
    src_rect.x0 = i64::min((src_fb.w - src_rect.w - 1).into(), src_rect.x0);
    src_rect.y0 = i64::min((src_fb.h - src_rect.h - 1).into(), src_rect.y0);

    dst_fb.copy_from_fb(src_fb, src_rect, &dst_rect, false);


    //
    // Vertical scrollbar

    let sbar_outer_rect = Rect { 
        x0: (dst_rect.w - SBAR_OUTER_W).into(),
        y0: 0,
        w: SBAR_OUTER_W,
        h: dst_rect.h,
    };

    let sbar_inner_rect = Rect { 
        x0: (dst_rect.w - SBAR_OUTER_W + (SBAR_OUTER_W - SBAR_INNER_W) / 2).into(),
        y0: (dst_rect.h as i64) * src_rect.y0 / (src_fb.h as i64),
        w: SBAR_INNER_W,
        h: dst_rect.h * src_rect.h / src_fb.h,
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

    draw_rect(dst_fb, &sbar_outer_rect, SBAR_OUTER_COLOR);
    draw_rect(dst_fb, &sbar_inner_rect, sbar_color);

    match p_state.left_clicked {
        true => {
            if sbar_hover {
                *dragging = true;
            }
        },
        false => *dragging = false,
    }

    //
    // Horizontal scrollbar

    // TODO

}

