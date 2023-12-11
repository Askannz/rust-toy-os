use crate::{Framebuffer, Color, Rect, blend_colors};

#[derive(Debug, Clone)]
pub struct ScreenPoint { pub x: i64, pub y: i64 }

pub fn draw_triangle(fb: &mut Framebuffer, tri: &[ScreenPoint; 3], color: Color) {

    let i = {
        if tri[0].y <= i64::min(tri[1].y, tri[2].y) { 0 }
        else if tri[1].y <= i64::min(tri[0].y, tri[2].y) { 1 }
        else { 2 }
    };

    let p0 = &tri[i];
    let p2 = &tri[(i + 1) % 3];
    let p1 = &tri[(i + 2) % 3];

    let y_half = i64::min(p1.y, p2.y);
    fill_half_triangle(fb, (p0, p1), (p0, p2), (p0.y, y_half), color);

    if p1.y < p2.y {
        fill_half_triangle(fb, (p1, p2), (p0, p2), (y_half, p2.y), color);
    } else {
        fill_half_triangle(fb, (p0, p1), (p2, p1), (y_half, p1.y), color);
    }
}

#[inline]
fn fill_half_triangle(
    fb: &mut Framebuffer,
    left: (&ScreenPoint, &ScreenPoint), right: (&ScreenPoint, &ScreenPoint),
    range: (i64, i64),
    color: Color
) {

    let (pl0, pl1) = left;
    let (pr0, pr1) = right;
    let (y_min, y_max) = range;

    if pl0.y == pl1.y || pr0.y == pr1.y { return; }

    let f_left = (pl1.x - pl0.x) as f32 / (pl1.y - pl0.y) as f32;
    let f_right = (pr1.x - pr0.x) as f32 / (pr1.y - pr0.y) as f32;

    for y in y_min..=y_max {
        let x_min = ((y - pl0.y) as f32 * f_left) as i64 + pl0.x;
        let x_max = ((y - pr0.y) as f32 * f_right) as i64 + pr0.x;
        if x_min <= x_max {
            let line_w = x_max - x_min + 1;
            fb.fill_line(x_min as u32, line_w as u32, y as u32, color);
        }
    }
}

pub fn draw_rect(fb: &mut Framebuffer, rect: &Rect, color: Color) {

    let rect = rect.intersection(&Rect { x0: 0, y0: 0, w: fb.w as u32, h: fb.h as u32});

    if let Some(Rect { x0, y0, w, h }) = rect {
        let (x0, y0) = (x0 as u32, y0 as u32);
        for y in y0..y0+h {
            fb.fill_line(x0, w, y, color);
        }
    }
}

pub fn blend_rect(fb: &mut Framebuffer, rect: &Rect, color: Color) {

    let rect = rect.intersection(&Rect { x0: 0, y0: 0, w: fb.w as u32, h: fb.h as u32});

    if let Some(Rect { x0, y0, w, h }) = rect {
        let (x0, y0) = (x0 as u32, y0 as u32);
        for y in y0..y0+h {
            for x in x0..x0+w {
                let current = fb.get_pixel(x, y);
                let new = blend_colors(color, current);
                fb.set_pixel(x, y, new);
            }
        }
    }
}

