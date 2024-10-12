use crate::geometry::{Point2D, Quad2D, Triangle2D, Vec2D};
use crate::{blend_colors, Color, FbViewMut, Rect};
use core::f32::consts::PI;
use num::Float;

pub fn draw_triangle<F: FbViewMut>(fb: &mut F, tri: &Triangle2D<i64>, color: Color, blend: bool) {
    const WIREFRAME: bool = false;
    match WIREFRAME {
        true => draw_triangle_with_wireframe(fb, tri, color, blend),
        false => internal_draw_triangle(fb, tri, color, blend),
    }
}

fn internal_draw_triangle<F: FbViewMut>(
    fb: &mut F,
    tri: &Triangle2D<i64>,
    color: Color,
    blend: bool,
) {
    let i = {
        if tri.points[0].y <= i64::min(tri.points[1].y, tri.points[2].y) {
            0
        } else if tri.points[1].y <= i64::min(tri.points[0].y, tri.points[2].y) {
            1
        } else {
            2
        }
    };

    let p0 = tri.points[i];
    let p2 = tri.points[(i + 1) % 3];
    let p1 = tri.points[(i + 2) % 3];

    let y_half = i64::min(p1.y, p2.y);
    draw_half_triangle(fb, (p0, p1), (p0, p2), (p0.y, y_half), color, blend);

    if p1.y < p2.y {
        draw_half_triangle(fb, (p1, p2), (p0, p2), (y_half + 1, p2.y), color, blend);
    } else {
        draw_half_triangle(fb, (p0, p1), (p2, p1), (y_half + 1, p1.y), color, blend);
    }
}

pub fn draw_quad<F: FbViewMut>(fb: &mut F, quad: &Quad2D<i64>, color: Color, blend: bool) {
    let (tri0, tri1) = quad.triangles();
    draw_triangle(fb, &tri0, color, blend);
    draw_triangle(fb, &tri1, color, blend);
}

pub fn draw_triangle_with_wireframe<F: FbViewMut>(
    fb: &mut F,
    tri: &Triangle2D<i64>,
    color: Color,
    blend: bool,
) {
    const F: f32 = 0.1;
    const WIREFRAME_COLOR: Color = Color::BLUE;

    let shrink_triangle = |tri: &Triangle2D<i64>| -> Triangle2D<i64> {
        let [p0, p1, p2] = core::array::from_fn(|i| tri.points[i].to_float());
        let center = Point2D::<f32> {
            x: (p0.x + p1.x + p2.x) / 3.0,
            y: (p0.y + p1.y + p2.y) / 3.0,
        };

        let p0 = (p0 + (center - p0) * F).round_to_int();
        let p1 = (p1 + (center - p1) * F).round_to_int();
        let p2 = (p2 + (center - p2) * F).round_to_int();

        Triangle2D {
            points: [p0, p1, p2],
        }
    };

    let tri_s = shrink_triangle(tri);

    internal_draw_triangle(fb, &tri, WIREFRAME_COLOR, false);
    internal_draw_triangle(fb, &tri_s, color, blend);
}

#[derive(Debug, Clone, Copy)]
pub enum ArcMode {
    Full,
    AngleRange(f32, f32),
    MultiAngleRange {
        inner: (f32, f32),
        outer: (f32, f32),
    },
}

pub fn draw_arc<F: FbViewMut>(
    fb: &mut F,
    center: Point2D<i64>,
    r_inner: f32,
    r_outer: f32,
    mode: ArcMode,
    px_per_pt: f32,
    color: Color,
    blend: bool,
) {
    let (a_in_min, a_in_max, a_out_min, a_out_max) = match mode {
        ArcMode::Full => (0.0, 2.0 * PI, 0.0, 2.0 * PI),
        ArcMode::AngleRange(amin, amax) => (amin, amax, amin, amax),
        ArcMode::MultiAngleRange {
            inner: (a_in_min, a_in_max),
            outer: (a_out_min, a_out_max),
        } => (a_in_min, a_in_max, a_out_min, a_out_max),
    };

    let outer_perimeter = 2.0 * PI * (r_outer as f32);

    let n_edge_pairs = f32::round(0.5 * outer_perimeter / px_per_pt) as usize;
    let n_outer_edges = 2 * n_edge_pairs;
    let n_outer_points = n_outer_edges + 1;

    let get_point = |i: usize, amin: f32, amax: f32, r: f32| {
        let a =
            amin + (amax - amin) * ((i % n_outer_points) as f32) / ((n_outer_points - 1) as f32);
        let v = Vec2D::<f32> {
            x: f32::cos(a),
            y: f32::sin(a),
        };
        center + (v * r).round_to_int()
    };

    for i in 0..n_edge_pairs {
        let p_out_0 = get_point(2 * i, a_out_min, a_out_max, r_outer);
        let p_out_1 = get_point(2 * i + 1, a_out_min, a_out_max, r_outer);
        let p_out_2 = get_point(2 * i + 2, a_out_min, a_out_max, r_outer);

        let p_in_0 = get_point(2 * i, a_in_min, a_in_max, r_inner);
        let p_in_2 = get_point(2 * i + 2, a_in_min, a_in_max, r_inner);

        let tri_a = Triangle2D::<i64> {
            points: [p_in_0, p_out_0, p_out_1],
        };
        let tri_b = Triangle2D::<i64> {
            points: [p_in_0, p_out_1, p_in_2],
        };
        let tri_c = Triangle2D::<i64> {
            points: [p_in_2, p_out_1, p_out_2],
        };

        draw_triangle(fb, &tri_a, color, blend);
        draw_triangle(fb, &tri_b, color, blend);
        draw_triangle(fb, &tri_c, color, blend);
    }
}

fn draw_half_triangle<F: FbViewMut>(
    fb: &mut F,
    left: (Point2D<i64>, Point2D<i64>),
    right: (Point2D<i64>, Point2D<i64>),
    range: (i64, i64),
    color: Color,
    blend: bool,
) {
    let (pl0, pl1) = left;
    let (pr0, pr1) = right;
    let (y_min, y_max) = range;

    if pl0.y == pl1.y || pr0.y == pr1.y {
        return;
    }

    let f_left = (pl1.x - pl0.x) as f32 / (pl1.y - pl0.y) as f32;
    let f_right = (pr1.x - pr0.x) as f32 / (pr1.y - pr0.y) as f32;

    for y in y_min..=y_max {
        let x_min = ((y - pl0.y) as f32 * f_left) as i64 + pl0.x;
        let x_max = ((y - pr0.y) as f32 * f_right) as i64 + pr0.x - 1; // -1 to avoid overlap on adjacent triangles
        if x_min <= x_max {
            let line_w = x_max - x_min + 1;
            fb.fill_line(x_min, line_w as u32, y, color, blend);
        }
    }
}

pub fn draw_rect<F: FbViewMut>(fb: &mut F, rect: &Rect, color: Color, blend: bool) {
    let (fb_w, fb_h) = fb.shape();
    let rect = rect.intersection(&Rect {
        x0: 0,
        y0: 0,
        w: fb_w,
        h: fb_h,
    });

    if let Some(Rect { x0, y0, w, h }) = rect {
        let h: i64 = h.into();
        for y in y0..y0 + h {
            fb.fill_line(x0, w, y, color, blend);
        }
    }
}

pub fn blend_rect<F: FbViewMut>(fb: &mut F, rect: &Rect, color: Color) {
    let (fb_w, fb_h) = fb.shape();
    let rect = rect.intersection(&Rect {
        x0: 0,
        y0: 0,
        w: fb_w,
        h: fb_h,
    });

    if let Some(Rect { x0, y0, w, h }) = rect {
        let (w, h): (i64, i64) = (w.into(), h.into());
        for y in y0..y0 + h {
            for x in x0..x0 + w {
                if let Some(curr_color) = fb.get_pixel(x, y) {
                    let new_color = blend_colors(color, curr_color);
                    fb.set_pixel(x, y, new_color);
                }
            }
        }
    }
}
