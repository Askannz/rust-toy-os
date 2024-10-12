extern crate alloc;

use applib::drawing::primitives::draw_quad;
use applib::geometry::{Point2D, Quad2D};
use applib::{Color, FbView, FbViewMut};
use num_traits::Float;

const COLORS: [Color; 6] = [
    Color::RED,
    Color::GREEN,
    Color::BLUE,
    Color::YELLOW,
    Color::FUCHSIA,
    Color::AQUA,
];
const ZOOM: f32 = 0.4;
const MOUSE_SENSITIVITY: f32 = 5.0;
const PI: f32 = 3.14159265359;
const NB_QUADS: usize = 6;

const BASE_QUAD: [Point; 4] = [
    Point {
        x: -1.0,
        y: -1.0,
        z: -1.0,
    },
    Point {
        x: 1.0,
        y: -1.0,
        z: -1.0,
    },
    Point {
        x: 1.0,
        y: 1.0,
        z: -1.0,
    },
    Point {
        x: -1.0,
        y: 1.0,
        z: -1.0,
    },
];

#[derive(Debug)]
pub struct Scene([Quad; NB_QUADS]);

pub fn load_scene() -> Scene {
    let mut geometry = [BASE_QUAD; NB_QUADS];

    rotate(&mut geometry[0], Axis::Y, 0.0 * PI / 2.0);
    rotate(&mut geometry[1], Axis::Y, 1.0 * PI / 2.0);
    rotate(&mut geometry[2], Axis::Y, 2.0 * PI / 2.0);
    rotate(&mut geometry[3], Axis::Y, 3.0 * PI / 2.0);

    rotate(&mut geometry[4], Axis::X, -PI / 2.0);
    rotate(&mut geometry[5], Axis::X, PI / 2.0);

    Scene(geometry)
}

pub fn draw_scene<F: FbViewMut>(fb: &mut F, scene: &Scene, xf: f32, yf: f32) {
    let mut geometry = scene.0.clone();

    let view_yaw = -xf * MOUSE_SENSITIVITY;
    let pitch = yf * MOUSE_SENSITIVITY;

    geometry.iter_mut().for_each(|quad| {
        rotate(quad, Axis::Y, view_yaw);
        rotate(quad, Axis::X, pitch);
    });

    rasterize(fb, &geometry);
}

fn rotate(poly: &mut Quad, axis: Axis, angle: f32) {
    let mat = match axis {
        Axis::X => [
            1.0,
            0.0,
            0.0,
            0.0,
            angle.cos(),
            -angle.sin(),
            0.0,
            angle.sin(),
            angle.cos(),
        ],

        Axis::Y => [
            angle.cos(),
            0.0,
            angle.sin(),
            0.0,
            1.0,
            0.0,
            -angle.sin(),
            0.0,
            angle.cos(),
        ],

        Axis::Z => [
            angle.cos(),
            -angle.sin(),
            0.0,
            angle.sin(),
            angle.cos(),
            0.0,
            0.0,
            0.0,
            1.0,
        ],
    };

    poly.iter_mut().for_each(|p| *p = matmul(&mat, p));
}

fn rasterize<F: FbViewMut>(fb: &mut F, geometry: &[Quad; NB_QUADS]) {
    let (fb_w, fb_h) = fb.shape();

    let w = fb_w as f32;
    let h = fb_h as f32;

    geometry_to_screen_space(w, h, geometry)
        .iter()
        .enumerate()
        .for_each(|(i, quad)| {
            let color = COLORS[i % COLORS.len()];
            rasterize_quad(fb, quad, color);
        });
}

fn rasterize_quad<F: FbViewMut>(fb: &mut F, quad: &Quad2D<i64>, color: Color) {
    if get_direction(quad) < 0 {
        return;
    }

    draw_quad(fb, quad, color, false);
}

fn get_direction(quad: &Quad2D<i64>) -> i64 {
    let [p0, p1, _p2, p3] = quad.points;
    (p1.x - p0.x) * (p3.y - p0.y) - (p3.x - p0.x) * (p1.y - p0.y)
}

fn geometry_to_screen_space(w: f32, h: f32, quads: &[Quad; NB_QUADS]) -> [Quad2D<i64>; NB_QUADS] {
    quads.clone().map(|quad| quad_to_screen_space(w, h, &quad))
}

fn quad_to_screen_space(w: f32, h: f32, quad: &Quad) -> Quad2D<i64> {
    let points: [Point2D<i64>; 4] = core::array::from_fn(|i| point_to_screen_space(w, h, &quad[i]));

    Quad2D { points }
}

fn point_to_screen_space(w: f32, h: f32, p: &Point) -> Point2D<i64> {
    let y_px = (h - 1.0) * (ZOOM * p.y + 1.0) / 2.0;
    let x_px = (h - 1.0) * (ZOOM * p.x + 1.0) / 2.0 + (w - h) / 2.0;
    let point = Point2D::<f32> { x: x_px, y: y_px };
    point.round_to_int()
}

fn matmul(m: &Matrix, vec: &Vector) -> Vector {
    let v = [vec.x, vec.y, vec.z];

    Vector {
        x: m[0] * v[0] + m[1] * v[1] + m[2] * v[2],
        y: m[3] * v[0] + m[4] * v[1] + m[5] * v[2],
        z: m[6] * v[0] + m[7] * v[1] + m[8] * v[2],
    }
}

#[derive(Debug, Clone)]
struct Vector {
    x: f32,
    y: f32,
    z: f32,
}

type Point = Vector;
type Quad = [Point; 4];
type Matrix = [f32; 9];

#[derive(Debug, Clone, Copy)]
#[allow(dead_code)]
enum Axis {
    X,
    Y,
    Z,
}
