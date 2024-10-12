use applib::drawing::primitives::draw_triangle;
use applib::geometry::{Point2D, Triangle2D};
use applib::{Color, FbViewMut};
use num_traits::Float;

const PI: f32 = 3.14159265359;

const HAND_BASE_HALF_W: f32 = 7.0;
const HAND_LEN: f32 = 80.0;

const DIVIDER: f32 = 60_000.0;

pub fn draw_chrono<F: FbViewMut>(fb: &mut F, time: f64) {
    let angle = (time as f32 % DIVIDER) / DIVIDER * 2.0 * PI;

    let (fb_w, fb_h) = fb.shape();

    let x0 = fb_w as f32 / 2.0;
    let y0 = fb_h as f32 / 2.0;

    let p0 = Point2D::<f32> {
        x: x0 + HAND_LEN * angle.cos(),
        y: y0 + HAND_LEN * angle.sin(),
    };

    let p1 = Point2D::<f32> {
        x: x0 - HAND_BASE_HALF_W * angle.sin(),
        y: y0 + HAND_BASE_HALF_W * angle.cos(),
    };

    let p2 = Point2D::<f32> {
        x: x0 + HAND_BASE_HALF_W * angle.sin(),
        y: y0 - HAND_BASE_HALF_W * angle.cos(),
    };

    let points = [p0.round_to_int(), p1.round_to_int(), p2.round_to_int()];
    let tri = Triangle2D::<i64> { points };

    draw_triangle(fb, &tri, Color::WHITE, false);
}
