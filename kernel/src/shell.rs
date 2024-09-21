use core::cell::RefCell;
use core::f32::consts::PI;
use core::f32;
use alloc::rc::Rc;
use alloc::vec::Vec;
use applib::input::InputState;
use applib::{Color, FbViewMut, Framebuffer, OwnedPixels, Rect};
use applib::uitk::{self, UiContext};
use applib::drawing::primitives::{draw_quad};
use applib::geometry::{Point2D, Vec2D, Triangle2D, Quad2D};
use num_traits::Float;
use crate::system::System;

pub fn pie_menu<F: FbViewMut>(
    uitk_context: &mut uitk::UiContext<F>,
    anchor: &mut Option<(i64, i64)>,
) {

    const INNER_RADIUS: f32 = 25.0;
    const OUTER_RADIUS: f32 = 150.0;
    const N_TRIANGLES: usize = 8;
    const COLOR_IDLE: Color = Color::rgba(0x44, 0x44, 0x44, 0xff);
    const COLOR_HOVER: Color = Color::rgba(0x88, 0x88, 0x88, 0xff);
    const GAP: f32 = 5.0;
    const OFFSET: f32 = 20.0;

    let pointer = &uitk_context.input_state.pointer;

    if !pointer.right_clicked {
        *anchor = None;
        return;
    }

    let (cx, cy) = anchor.get_or_insert((pointer.x, pointer.y));

    let center = Point2D::<i64> { x: *cx, y: *cy };
    let pointer = Point2D::<i64> { x: pointer.x, y: pointer.y };

    let angles: [f32; N_TRIANGLES] = core::array::from_fn(|i| {
        2.0 * PI * (i as f32 - 0.5) / (N_TRIANGLES as f32)
    });

    let n = angles.len();
    for i in 0..n {

        let a0 = angles[i];
        let a1 = angles[(i + 1) % n];

        let v0 = Vec2D::<f32> { x: f32::cos(a0), y: f32::sin(a0) };
        let v1 = Vec2D::<f32> { x: f32::cos(a1), y: f32::sin(a1) };
        let v_normal = v1 - v0;
        let v_tangent = (v0 + v1) * 0.5;

        let p_in_0 = center + (v0 * INNER_RADIUS).round_to_int();
        let p_out_0 = center + (v0 * OUTER_RADIUS).round_to_int();
        let p_in_1 = center + (v1 * INNER_RADIUS).round_to_int();
        let p_out_1 = center + (v1 * OUTER_RADIUS).round_to_int();

        let v_gap = (v_normal * GAP).round_to_int();
        let v_offset = (v_tangent * OFFSET).round_to_int();

        let mut quad = Quad2D::<i64> {
            points: [
                p_in_0 + v_gap,
                p_out_0 + v_gap,
                p_out_1 - v_gap,
                p_in_1 - v_gap,
            ]
        };

        let is_selected = quad.check_is_inside(pointer);

        let color = match is_selected {
            true => COLOR_HOVER,
            false => COLOR_IDLE,
        };

        if is_selected {
            quad.points.iter_mut().for_each(|p| *p = *p + v_offset);
        }

        draw_quad(uitk_context.fb, &quad, color);
    }
}
