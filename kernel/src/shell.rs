use core::cell::RefCell;
use core::f32::consts::PI;
use core::f32;
use alloc::rc::Rc;
use alloc::vec::Vec;
use applib::input::InputState;
use applib::{Color, FbView, FbViewMut, Framebuffer, OwnedPixels, Rect};
use applib::uitk::{self, UiContext};
use applib::drawing::primitives::{draw_arc, draw_quad, ArcMode};
use applib::geometry::{Point2D, Vec2D, Triangle2D, Quad2D};
use num_traits::Float;
use crate::system::System;

pub struct PieMenuEntry {
    pub icon: &'static Framebuffer<OwnedPixels>,
    pub bg_color: Color,
}

pub fn pie_menu<F: FbViewMut>(
    uitk_context: &mut uitk::UiContext<F>,
    entries: &[PieMenuEntry],
    anchor: &mut Option<(i64, i64)>,
) {

    const INNER_RADIUS: f32 = 100.0;
    const OUTER_RADIUS: f32 = 150.0;
    const COLOR_HOVER: Color = Color::rgba(0x88, 0x88, 0x88, 0xff);
    const COLOR_CLICKED: Color = Color::rgba(200, 200, 200, 0xff);
    const GAP: f32 = 2.0;
    const OFFSET_HOVER: f32 = 10.0;
    const OFFSET_CLICKED: f32 = 20.0;
    const ARC_PX_PER_PT: f32 = 100.0;

    let pointer = &uitk_context.input_state.pointer;

    if !pointer.right_clicked {
        *anchor = None;
        return;
    }

    let (cx, cy) = anchor.get_or_insert((pointer.x, pointer.y));

    let center = Point2D::<i64> { x: *cx, y: *cy };
    let pointer = Point2D::<i64> { x: pointer.x, y: pointer.y };

    let n = entries.len();

    let r_middle = (INNER_RADIUS + OUTER_RADIUS) * 0.5;

    for (i, entry) in entries.iter().enumerate() {

        let get_angle = |i| 2.0 * PI * (i as f32) / (n as f32);

        let a0 = get_angle(i);
        let a1 = get_angle(i + 1);

        let v0 = Vec2D::<f32> { x: f32::cos(a0), y: f32::sin(a0) };
        let v1 = Vec2D::<f32> { x: f32::cos(a1), y: f32::sin(a1) };

        let a_middle = (a0 + a1) / 2.0;
        let v_bisect = Vec2D::<f32> { x: f32::cos(a_middle), y: f32::sin(a_middle) };

        let v_cursor = (pointer - center).to_float();

        let is_hover = v_cursor.cross(v0) < 0.0 && v_cursor.cross(v1) > 0.0;

        let (offset, color) = match is_hover {
            false => (0.0, entry.bg_color),
            true => match uitk_context.input_state.pointer.left_clicked {
                false => (OFFSET_HOVER, COLOR_HOVER),
                true => (OFFSET_CLICKED, COLOR_CLICKED),
            }
        };

        let v_offset = (v_bisect * offset).round_to_int();

        let p_icon = center + (v_bisect * r_middle).round_to_int() + v_offset;
        let p_arc = center + v_offset;

        let inner_angle_gap = GAP / INNER_RADIUS;
        let outer_angle_gap = GAP / OUTER_RADIUS;
        let arc_mode = ArcMode::MultiAngleRange { 
            inner: (a0 + inner_angle_gap, a1 - inner_angle_gap),
            outer: (a0 + outer_angle_gap, a1 - outer_angle_gap),
        };
        draw_arc(uitk_context.fb, p_arc, INNER_RADIUS, OUTER_RADIUS, arc_mode, ARC_PX_PER_PT, color);

        let (icon_w, icon_h) = entry.icon.shape();
        let x0_icon = p_icon.x - (icon_w / 2) as i64;
        let y0_icon = p_icon.y - (icon_h / 2) as i64;
        uitk_context.fb.copy_from_fb(entry.icon, (x0_icon, y0_icon), true);
    }
}
