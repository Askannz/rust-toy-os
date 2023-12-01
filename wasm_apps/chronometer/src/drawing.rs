use num_traits::Float;
use applib::{Color, Framebuffer};
use applib::drawing::{ScreenPoint, draw_triangle};


const PI: f64 = 3.14159265359;
const COLOR: Color = Color::from_rgba(0xff, 0xff, 0xff, 0xff);

const HAND_BASE_HALF_W: f64 = 7.0;
const HAND_LEN: f64 = 80.0;

const DIVIDER: f64 = 60_000.0;

pub fn draw_chrono(fb: &mut Framebuffer, time: f64) {

    let angle = (time % DIVIDER) / DIVIDER  * 2.0 * PI;

    let x0 = fb.w as f64 / 2.0;
    let y0 = fb.h as f64 / 2.0;

    let p0x = (x0 + HAND_LEN * angle.cos()).round();
    let p0y = (y0 + HAND_LEN * angle.sin()).round();
    let p1x = (x0 - HAND_BASE_HALF_W * angle.sin()).round();
    let p1y = (y0 + HAND_BASE_HALF_W * angle.cos()).round();
    let p2x = (x0 + HAND_BASE_HALF_W * angle.sin()).round();
    let p2y = (y0 - HAND_BASE_HALF_W * angle.cos()).round();

    let tri = [
        ScreenPoint { x: p0x as i64, y: p0y as i64 },
        ScreenPoint { x: p1x as i64, y: p1y as i64 },
        ScreenPoint { x: p2x as i64, y: p2y as i64 },
    ];

    draw_triangle(fb, &tri, COLOR);
}

