use core::cell::RefCell;
use core::f32::consts::PI;
use core::f32;
use alloc::string::String;
use alloc::rc::Rc;
use alloc::vec::Vec;
use applib::input::InputState;
use applib::{Color, FbView, FbViewMut, Framebuffer, OwnedPixels, Rect};
use applib::uitk::{self, UiContext};
use applib::drawing::primitives::{draw_arc, draw_quad, ArcMode};
use applib::drawing::text::{draw_str, Font};
use applib::geometry::{Point2D, Vec2D, Triangle2D, Quad2D};
use num_traits::Float;
use crate::system::System;

pub struct PieMenuEntry {
    pub icon: &'static Framebuffer<OwnedPixels>,
    pub bg_color: Color,
    pub text: String,
    pub text_color: Color,
    pub font: &'static Font,
}

pub fn pie_menu<F: FbViewMut>(
    uitk_context: &mut uitk::UiContext<F>,
    entries: &[PieMenuEntry],
    center: Point2D<i64>,
) -> Option<usize> {

    const INNER_RADIUS: f32 = 50.0;
    const OUTER_RADIUS: f32 = 100.0;
    const DEADZONE_RADIUS: f32 = 25.0;
    const GAP: f32 = 2.0;
    const OFFSET_HOVER: f32 = 10.0;
    const ARC_PX_PER_PT: f32 = 20.0;
    const TEXT_OFFSET: f32 = 10.0;
    const COLOR_HOVER_OVERLAY: Color = Color::rgba(255, 255, 255, 128);

    let pointer = &uitk_context.input_state.pointer;

    let pointer = Point2D::<i64> { x: pointer.x, y: pointer.y };

    let n = entries.len();

    let r_middle = (INNER_RADIUS + OUTER_RADIUS) * 0.5;

    let mut selected_entry = None;

    for (i, entry) in entries.iter().enumerate() {

        let get_angle = |i| 2.0 * PI * (i as f32) / (n as f32);

        let a0 = get_angle(i);
        let a1 = get_angle(i + 1);

        let v0 = Vec2D::<f32> { x: f32::cos(a0), y: f32::sin(a0) };
        let v1 = Vec2D::<f32> { x: f32::cos(a1), y: f32::sin(a1) };

        let a_middle = (a0 + a1) / 2.0;
        let v_bisect = Vec2D::<f32> { x: f32::cos(a_middle), y: f32::sin(a_middle) };

        let v_cursor = (pointer - center).to_float();

        let center_dist = v_cursor.norm();

        let is_hover = v_cursor.cross(v0) < 0.0 && v_cursor.cross(v1) > 0.0 && center_dist > DEADZONE_RADIUS;

        let (offset, text_visible) = match is_hover {
            false => (0.0, false),
            true => {
                if uitk_context.input_state.pointer.left_clicked {
                    selected_entry = Some(i);
                }
                (OFFSET_HOVER, true)
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

        draw_arc(uitk_context.fb, p_arc, INNER_RADIUS, OUTER_RADIUS, arc_mode, ARC_PX_PER_PT, entry.bg_color, false);

        let (icon_w, icon_h) = entry.icon.shape();
        let x0_icon = p_icon.x - (icon_w / 2) as i64;
        let y0_icon = p_icon.y - (icon_h / 2) as i64;
        uitk_context.fb.copy_from_fb(entry.icon, (x0_icon, y0_icon), true);

        if is_hover {
            draw_arc(uitk_context.fb, p_arc, INNER_RADIUS, OUTER_RADIUS, arc_mode, ARC_PX_PER_PT, COLOR_HOVER_OVERLAY, true);
        }

        if text_visible {
            let p_text = center + (v_bisect * (OUTER_RADIUS + TEXT_OFFSET)).round_to_int() + v_offset;
            let (text_w, text_h) = compute_text_bbox(&entry.text, entry.font);
            let x0_text = match v_bisect.x > 0.0 {
                true => p_text.x,
                false => p_text.x - text_w as i64,
            };
            let y0_text = p_text.y - (text_h / 2) as i64;
            draw_str(uitk_context.fb, &entry.text, x0_text, y0_text, entry.font, entry.text_color, None);
        }
    }

    selected_entry
}

fn compute_text_bbox(s: &str, font: &Font) -> (u32, u32) {
    let w = font.char_w * s.len();
    let h = font.char_h;
    (w as u32, h as u32)
}
