use alloc::borrow::ToOwned;
use alloc::string::String;
use alloc::vec::Vec;
use applib::drawing::primitives::{draw_arc, ArcMode};
use applib::drawing::text::{draw_str, Font};
use applib::geometry::{Point2D, Vec2D};
use applib::uitk::{self};
use applib::{Color, FbView, FbViewMut, Framebuffer, OwnedPixels, StyleSheet};
use core::f32;
use core::f32::consts::PI;
use num_traits::Float;

pub enum PieMenuEntry {
    Button {
        icon: &'static Framebuffer<OwnedPixels>,
        color: Color,
        text: String,
        text_color: Color,
        font: &'static Font,
        weight: f32,
    },
    Spacer {
        color: Color,
        weight: f32,
    },
}

impl PieMenuEntry {
    fn weight(&self) -> f32 {
        match self {
            PieMenuEntry::Button { weight, .. } => *weight,
            PieMenuEntry::Spacer { weight, .. } => *weight,
        }
    }
    fn color(&self) -> Color {
        match self {
            PieMenuEntry::Button { color, .. } => *color,
            PieMenuEntry::Spacer { color, .. } => *color,
        }
    }
}

// We have to defer the draw operation of the pie menu,
// because it's supposed to be drawn above the rest (app windows, etc)
pub struct PieDrawCalls {
    calls: Vec<DrawCall>
}

pub fn pie_menu<'a, F: FbViewMut>(
    uitk_context: &mut uitk::UiContext<F>,
    entries: &'a [PieMenuEntry],
    center: Point2D<i64>,
) -> (Option<&'a str>, PieDrawCalls) {
    const INNER_RADIUS: f32 = 50.0;
    const OUTER_RADIUS: f32 = 100.0;
    const DEADZONE_INNER_RADIUS: f32 = 25.0;
    const DEADZONE_OUTER_RADIUS: f32 = 200.0;
    const GAP: f32 = 2.0;
    const OFFSET_HOVER: f32 = 10.0;
    const ARC_PX_PER_PT: f32 = 20.0;
    const TEXT_OFFSET: f32 = 10.0;

    let pointer = &uitk_context.input_state.pointer;
    let stylesheet = &uitk_context.stylesheet;

    let pointer = Point2D::<i64> {
        x: pointer.x,
        y: pointer.y,
    };

    let r_middle = (INNER_RADIUS + OUTER_RADIUS) * 0.5;

    let total_weight: f32 = entries.iter().map(|entry| entry.weight()).sum();

    let mut selected_entry = None;
    let mut a0 = 0.0;
    let mut draw_calls  = PieDrawCalls { calls: Vec::new() };

    for entry in entries.iter() {
        let delta_angle = 2.0 * PI * entry.weight() / total_weight;
        let a1 = a0 + delta_angle;

        let v0 = Vec2D::<f32> {
            x: f32::cos(a0),
            y: f32::sin(a0),
        };
        let v1 = Vec2D::<f32> {
            x: f32::cos(a1),
            y: f32::sin(a1),
        };

        let a_middle = (a0 + a1) / 2.0;
        let v_bisect = Vec2D::<f32> {
            x: f32::cos(a_middle),
            y: f32::sin(a_middle),
        };

        let v_cursor = (pointer - center).to_float();

        let center_dist = v_cursor.norm();

        let is_hovered = match entry {
            PieMenuEntry::Spacer { .. } => false,
            PieMenuEntry::Button { text, .. } => {
                let is_hovered = v_cursor.cross(v0) < 0.0
                    && v_cursor.cross(v1) > 0.0
                    && center_dist > DEADZONE_INNER_RADIUS
                    && center_dist < DEADZONE_OUTER_RADIUS;

                if is_hovered {
                    selected_entry = Some(text.as_str());
                }

                is_hovered
            }
        };

        let v_offset = match is_hovered {
            true => (v_bisect * OFFSET_HOVER).round_to_int(),
            false => Vec2D::zero(),
        };

        let p_icon = center + (v_bisect * r_middle).round_to_int() + v_offset;
        let p_arc = center + v_offset;

        let inner_angle_gap = GAP / INNER_RADIUS;
        let outer_angle_gap = GAP / OUTER_RADIUS;
        let arc_mode = ArcMode::MultiAngleRange {
            inner: (a0 + inner_angle_gap, a1 - inner_angle_gap),
            outer: (a0 + outer_angle_gap, a1 - outer_angle_gap),
        };

        draw_calls.calls.push(DrawCall::Arc {
            center: p_arc,
            r_inner: INNER_RADIUS,
            r_outer: OUTER_RADIUS,
            mode: arc_mode,
            px_per_pt: ARC_PX_PER_PT,
            color: entry.color(),
            blend: false
        });

        if let PieMenuEntry::Button {
            icon,
            text,
            text_color,
            font,
            ..
        } = entry
        {
            let (icon_w, icon_h) = icon.shape();
            let x0_icon = p_icon.x - (icon_w / 2) as i64;
            let y0_icon = p_icon.y - (icon_h / 2) as i64;

            draw_calls.calls.push(DrawCall::Icon {
                icon: *icon,
                x0: x0_icon,
                y0: y0_icon
            });

            if is_hovered {

                draw_calls.calls.push(DrawCall::Arc {
                    center: p_arc,
                    r_inner: INNER_RADIUS,
                    r_outer: OUTER_RADIUS,
                    mode: arc_mode,
                    px_per_pt: ARC_PX_PER_PT,
                    color: stylesheet.colors.hover_overlay,
                    blend: true
                });

                let p_text =
                    center + (v_bisect * (OUTER_RADIUS + TEXT_OFFSET)).round_to_int() + v_offset;
                let (text_w, text_h) = compute_text_bbox(text, font);
                let x0_text = match v_bisect.x > 0.0 {
                    true => p_text.x,
                    false => p_text.x - text_w as i64,
                };
                let y0_text = p_text.y - (text_h / 2) as i64;

                draw_calls.calls.push(DrawCall::Text { 
                    s: text.to_owned(),
                    x0: x0_text,
                    y0: y0_text,
                    font: font,
                    color: *text_color,
                    bg_color: None
                });
            }
        }

        a0 = a1;
    }

    (selected_entry, draw_calls)
}

fn compute_text_bbox(s: &str, font: &Font) -> (u32, u32) {
    let w = font.char_w * s.len();
    let h = font.char_h;
    (w as u32, h as u32)
}

impl PieDrawCalls {
    pub fn draw<F: FbViewMut>(self, fb: &mut F) {
        for call in self.calls.into_iter() {
            match call {

                DrawCall::Arc { 
                    center,
                    r_inner,
                    r_outer,
                    mode,
                    px_per_pt, 
                    color,
                    blend 
                } => draw_arc(fb, center, r_inner, r_outer, mode, px_per_pt, color, blend),

                DrawCall::Icon { 
                    icon,
                    x0,
                    y0
                } => fb.copy_from_fb(icon, (x0, y0), true),

                DrawCall::Text { 
                    s,
                    x0,
                    y0,
                    font,
                    color,
                    bg_color
                } => draw_str(fb, &s, x0, y0, font, color, bg_color),
            }
        }
    }
}


enum DrawCall {
    Arc {
        center: Point2D<i64>,
        r_inner: f32,
        r_outer: f32,
        mode: ArcMode,
        px_per_pt: f32,
        color: Color,
        blend: bool,
    },
    Icon {
        icon: &'static Framebuffer<OwnedPixels>,
        x0: i64,
        y0: i64,
    },
    Text {
        s: String,
        x0: i64,
        y0: i64,
        font: &'static Font,
        color: Color,
        bg_color: Option<Color>,
    }
}
