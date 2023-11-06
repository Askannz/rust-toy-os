//use micromath::F32Ext;
use num_traits::Float;
use applib::{Color, FrameBufRegion};

const ZOOM: f32 = 0.2;
const PI: f32 = 3.14159265359;
const COLOR: Color = Color(0xff, 0xff, 0xff);


pub fn draw_chrono(fb: &mut FrameBufRegion, time: u64) {

    const DIVIDER: u64 = 60_000;
    let angle = (((time % DIVIDER) as f32) / (DIVIDER as f32)) * 2.0 * PI;

    let quad = [
        Point { x: -0.5, y: 0.0, z: 0.0 },
        Point { x: 0.5, y: 0.0, z: 0.0 },
        Point { x: 0.1, y: 4.0, z: 0.0 },
        Point { x: -0.1, y: 4.0, z: 0.0 }
    ];

    let quad = rotate(&quad, Axis::Z, angle);

    rasterize(fb, &quad);
}

fn rotate(poly: &Quad, axis: Axis, angle: f32) -> Quad {

    let mat = match axis {
        Axis::X => [
            1.0, 0.0, 0.0,
            0.0, angle.cos(), -angle.sin(),
            0.0, angle.sin(), angle.cos()
        ],

        Axis::Y => [
            angle.cos(), 0.0, angle.sin(),
            0.0, 1.0, 0.0,
            -angle.sin(), 0.0, angle.cos()
        ],

        Axis::Z => [
            angle.cos(), -angle.sin(), 0.0,
            angle.sin(), angle.cos(), 0.0,
            0.0, 0.0, 1.0
        ]
    };

    let mut new_poly: Quad = [Point {x: 0.0, y: 0.0, z: 0.0}; 4];

    for (i, p) in poly.iter().enumerate() {
        let new_p = matmul(&mat, p);
        new_poly[i] = new_p;
    }

    new_poly
}

fn rasterize(fb: &mut FrameBufRegion, poly: &Quad) {

    let (mut min_x, mut min_y) = (0.0, 0.0);
    let (mut max_x, mut max_y) = (0.0, 0.0);

    for p in poly.iter() {
        min_x = f32::min(min_x, p.x);
        min_y = f32::min(min_y, p.y);
        max_x = f32::max(max_x, p.x);
        max_y = f32::max(max_y, p.y);
    }

    let w = fb.rect.w as f32;
    let h = fb.rect.h as f32;

    for x_px in 0..fb.rect.w {
        for y_px in 0..fb.rect.h {

            let p = {
   
                let x_px = x_px as f32;
                let y_px = y_px as f32;

                let rx = 2.0 * (x_px - (w - h) / 2.0) / (h - 1.0);
                let ry = 2.0 * y_px / (h - 1.0);

                Point {
                    x: (rx - 1.0) / ZOOM,
                    y: (ry - 1.0) / ZOOM,
                    z: 0.0
                }
            };

            if p.x < min_x || p.x > max_x || p.y < min_y || p.y > max_y {
                continue;
            }

            if test_in_poly(&poly, &p) {
                fb.set_pixel(x_px, y_px, &COLOR);
            }
        }
    }

}

fn test_in_poly(poly: &Quad, p: &Point) -> bool {

    let n = poly.len();

    for i1 in 0..n {

        let p1 = poly[i1];
        let p2 = poly[(i1 + 1) % n];

        let d = (p2.x - p1.x) * (p.y - p1.y) - (p2.y - p1.y) * (p.x - p1.x);

        if d < 0.0 { return false; }
    }

    return true;
}

fn matmul(m: &Matrix, vec: &Vector) -> Vector {

    let v = [vec.x, vec.y, vec.z];

    Vector {
        x: m[0] * v[0] + m[1] * v[1] + m[2] * v[2],
        y: m[3] * v[0] + m[4] * v[1] + m[5] * v[2],
        z: m[6] * v[0] + m[7] * v[1] + m[8] * v[2]
    }
}

#[derive(Debug, Clone, Copy)]
struct Vector { x: f32, y: f32, z: f32 }
type Point = Vector;
type Quad = [Point; 4];
type Matrix = [f32; 9];

#[derive(Debug, Clone, Copy)]
#[allow(dead_code)]
enum Axis { X, Y, Z }
