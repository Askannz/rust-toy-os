use num_traits::Float;
use applib::{Color, Framebuffer};

const COLORS: [Color; 6] = [
    Color(0xff, 0x00, 0x00),
    Color(0x00, 0xff, 0x00),
    Color(0x00, 0x00, 0xff),
    Color(0xff, 0xff, 0x00),
    Color(0xff, 0x00, 0xff),
    Color(0x00, 0xff, 0xff),
];
const ZOOM: f32 = 0.4;
const MOUSE_SENSITIVITY: f32 = 5.0;
const PI: f32 = 3.14159265359;
const NB_QUADS: usize = 6;

const BASE_QUAD: [Point; 4] = [
    Point { x: -1.0, y: -1.0, z: -1.0 },
    Point { x: 1.0, y: -1.0, z: -1.0 },
    Point { x: 1.0, y: 1.0, z: -1.0 },
    Point { x: -1.0, y: 1.0, z: -1.0 }
];


pub fn draw_cube(fb: &mut Framebuffer, xf: f32, yf: f32) {

    let geometry = [
        rotate(&BASE_QUAD, Axis::Y, 0.0 * PI / 2.0),
        rotate(&BASE_QUAD, Axis::Y, 1.0 * PI / 2.0),
        rotate(&BASE_QUAD, Axis::Y, 2.0 * PI / 2.0),
        rotate(&BASE_QUAD, Axis::Y, 3.0 * PI / 2.0),

        rotate(&BASE_QUAD, Axis::X, - PI / 2.0),
        rotate(&BASE_QUAD, Axis::X, PI / 2.0)
    ];

    let view_yaw = -xf * MOUSE_SENSITIVITY;
    let pitch = yf * MOUSE_SENSITIVITY;
    
    let geometry = geometry.map(|mut quad| {
        quad = rotate(&quad, Axis::Y, view_yaw);
        quad = rotate(&quad, Axis::X, pitch);
        quad
    });

    rasterize(fb, &geometry);
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

    poly.clone().map(|p| matmul(&mat, &p))
}

fn rasterize(fb: &mut Framebuffer, geometry: &[Quad; NB_QUADS]) {

    let w = fb.rect.w as f32;
    let h = fb.rect.h as f32;

    geometry_to_screen_space(w, h, geometry)
        .iter()
        .enumerate()
        .for_each(|(i, quad)| {
            let color = &COLORS[i % COLORS.len()];
            rasterize_quad(fb, quad, color);
        });
}

fn rasterize_quad(fb: &mut Framebuffer, quad: &IntQuad, color: &Color) {

    if get_direction(quad) < 0 { return; }

    let [p0, p1, p2, p3] = quad;
    rasterize_triangle(fb, [p0, p1, p2], color);
    rasterize_triangle(fb, [p2, p3, p0], color);

}

fn rasterize_triangle(fb: &mut Framebuffer, tri: [&IntPoint; 3], color: &Color) {

    let (i, p0) = tri.iter().enumerate().min_by_key(|(_i, p)| p.y).unwrap();
    let p2 = tri[(i + 1) % 3];
    let p1 = tri[(i + 2) % 3];

    let f_left = (p1.x - p0.x) as f32 / (p1.y - p0.y) as f32;
    let f_right = (p2.x - p0.x) as f32 / (p2.y - p0.y) as f32;
    
    let y_max = i64::min(p1.y, p2.y);

    for y in p0.y..=y_max {
        let x_min = ((y - p0.y) as f32 * f_left) as i64 + p0.x;
        let x_max = ((y - p0.y) as f32 * f_right) as i64 + p0.x;
        for x in x_min..=x_max {
            fb.set_pixel(x as u32, y as u32, color);
        }
    }

    if p1.y < p2.y {
        let f_bottom = (p2.x - p1.x) as f32 / (p2.y - p1.y) as f32;
        for y in y_max..=p2.y {
            let x_min = ((y - p1.y) as f32 * f_bottom) as i64 + p1.x;
            let x_max = ((y - p0.y) as f32 * f_right) as i64 + p0.x;
            for x in x_min..=x_max {
                fb.set_pixel(x as u32, y as u32, color);
            }
        }
    } else {
        let f_bottom = (p1.x - p2.x) as f32 / (p1.y - p2.y) as f32;
        for y in y_max..=p1.y {
            let x_min = ((y - p0.y) as f32 * f_left) as i64 + p0.x;
            let x_max = ((y - p2.y) as f32 * f_bottom) as i64 + p2.x;
            for x in x_min..=x_max {
                fb.set_pixel(x as u32, y as u32, color);
            }
        }
    }

}


fn get_direction(quad: &IntQuad) -> i64 {

    let p0 = &quad[0];
    let p1 = &quad[1];
    let p3 = &quad[3];

    (p1.x - p0.x) * (p3.y - p0.y) - (p3.x - p0.x) * (p1.y - p0.y)
}

fn geometry_to_screen_space(w: f32, h: f32, quads: &[Quad; NB_QUADS]) -> [IntQuad; NB_QUADS] {
    quads.clone().map(|quad| quad_to_screen_space(w, h, &quad))
}

fn quad_to_screen_space(w: f32, h: f32, quad: &Quad) -> IntQuad {
    quad.clone().map(|p| point_to_screen_space(w, h, &p))
}

fn point_to_screen_space(w: f32, h: f32, p: &Point) -> IntPoint {
    let y_px = (h - 1.0) * (ZOOM * p.y + 1.0) / 2.0;
    let x_px = (h - 1.0) * (ZOOM * p.x + 1.0) / 2.0 + (w - h) / 2.0;
    IntPoint { x: x_px as i64, y: y_px as i64 }
}

fn matmul(m: &Matrix, vec: &Vector) -> Vector {

    let v = [vec.x, vec.y, vec.z];

    Vector {
        x: m[0] * v[0] + m[1] * v[1] + m[2] * v[2],
        y: m[3] * v[0] + m[4] * v[1] + m[5] * v[2],
        z: m[6] * v[0] + m[7] * v[1] + m[8] * v[2]
    }
}

#[derive(Debug, Clone)]
struct Vector { x: f32, y: f32, z: f32 }

#[derive(Debug, Clone)]
struct IntVector { x: i64, y: i64 }

type Point = Vector;
type IntPoint = IntVector;
type Quad = [Point; 4];
type IntQuad = [IntPoint; 4];
type Matrix = [f32; 9];

#[derive(Debug, Clone, Copy)]
#[allow(dead_code)]
enum Axis { X, Y, Z }
