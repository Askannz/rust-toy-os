use crate::{blend_colors, Color, FbViewMut, Rect};
use crate::geometry::{Triangle2D, Quad2D, Point2D};

pub fn draw_triangle<F: FbViewMut>(fb: &mut F, tri: &Triangle2D<i64>, color: Color) {
    let i = {
        if tri.points[0].y <= i64::min(tri.points[1].y, tri.points[2].y) {
            0
        } else if tri.points[1].y <= i64::min(tri.points[0].y, tri.points[2].y) {
            1
        } else {
            2
        }
    };

    let p0 = &tri.points[i];
    let p2 = &tri.points[(i + 1) % 3];
    let p1 = &tri.points[(i + 2) % 3];

    let y_half = i64::min(p1.y, p2.y);
    fill_half_triangle(fb, (p0, p1), (p0, p2), (p0.y, y_half), color);

    if p1.y < p2.y {
        fill_half_triangle(fb, (p1, p2), (p0, p2), (y_half, p2.y), color);
    } else {
        fill_half_triangle(fb, (p0, p1), (p2, p1), (y_half, p1.y), color);
    }
}

pub fn draw_quad<F: FbViewMut>(fb: &mut F, quad: &Quad2D<i64>, color: Color) {
    let (tri0, tri1) = quad.triangles();
    draw_triangle(fb, &tri0, color);
    draw_triangle(fb, &tri1, color);
}


#[inline]
fn fill_half_triangle<F: FbViewMut>(
    fb: &mut F,
    left: (&Point2D<i64>, &Point2D<i64>),
    right: (&Point2D<i64>, &Point2D<i64>),
    range: (i64, i64),
    color: Color,
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
        let x_max = ((y - pr0.y) as f32 * f_right) as i64 + pr0.x;
        if x_min <= x_max {
            let line_w = x_max - x_min + 1;
            fb.fill_line(x_min, line_w as u32, y, color);
        }
    }
}

pub fn draw_rect<F: FbViewMut>(fb: &mut F, rect: &Rect, color: Color) {
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
            fb.fill_line(x0, w, y, color);
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
