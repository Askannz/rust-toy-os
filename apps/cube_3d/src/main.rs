#![no_std]
#![no_main]

use core::panic::PanicInfo;

use core::cmp::Ordering;
use micromath::F32Ext;

use applib::{AppHandle, Color, FrameBufSlice};

const COLORS: [Color; 6] = [
    Color(0xff, 0x00, 0x00),
    Color(0x00, 0xff, 0x00),
    Color(0x00, 0x00, 0xff),
    Color(0xff, 0xff, 0x00),
    Color(0xff, 0x00, 0xff),
    Color(0x00, 0xff, 0xff),
];
const ZOOM: f32 = 0.2;
const MOUSE_SENSITIVITY: f32 = 5.0;
const PI: f32 = 3.14159265359;
const NB_QUADS: usize = 6;


#[no_mangle]
pub extern "C" fn efi_main(handle: &mut AppHandle) {

    let win_rect = &handle.app_rect;
    let pointer = &handle.system_state.pointer;

    let xf = (pointer.x as f32) / ((win_rect.w - 1) as f32);
    let yf = (pointer.y as f32) / ((win_rect.h - 1) as f32);

    draw_cube(&mut handle.app_framebuffer, xf, yf);
}


/// This function is called on panic.
#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {}
}

pub fn draw_cube(fb: &mut FrameBufSlice, xf: f32, yf: f32) {

    let base_quad = [
        Point { x: -1.0, y: -1.0, z: -1.0 },
        Point { x: 1.0, y: -1.0, z: -1.0 },
        Point { x: 1.0, y: 1.0, z: -1.0 },
        Point { x: -1.0, y: 1.0, z: -1.0 }
    ];

    let zero_point = Point {x: 0.0, y: 0.0, z: 0.0};
    let zero_quad = [zero_point; 4];
    let mut geometry = [zero_quad; NB_QUADS];
    for i in 0..4 {
        let i_f = i as f32;
        geometry[i] = rotate(&base_quad, Axis::Y, i_f * PI / 2.0);
    }
    geometry[4] = rotate(&base_quad, Axis::X, - PI / 2.0);
    geometry[5] = rotate(&base_quad, Axis::X, PI / 2.0);

    let view_yaw = xf * MOUSE_SENSITIVITY;
    let pitch = yf * MOUSE_SENSITIVITY;

    geometry.iter_mut().for_each(|quad| {
        *quad = rotate(quad, Axis::Y, view_yaw);
    });

    geometry.iter_mut().for_each(|quad| {
        *quad = rotate(quad, Axis::X, pitch);
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

    let mut new_poly: Quad = [Point {x: 0.0, y: 0.0, z: 0.0}; 4];

    for (i, p) in poly.iter().enumerate() {
        let new_p = matmul(&mat, p);
        new_poly[i] = new_p;
    }

    new_poly
}

fn rasterize(fb: &mut FrameBufSlice, geometry: &[Quad; NB_QUADS]) {

    for (i, poly) in geometry.iter().enumerate() {
        let color = &COLORS[i % COLORS.len()];
        rasterize_poly(fb, poly, &color);
    }

}

fn rasterize_poly(fb: &mut FrameBufSlice, poly: &Quad, color: &Color) {

    let cmp_f = |a: &f32, b: &f32| { a.partial_cmp(b).unwrap_or(Ordering::Equal) };
    let min_x = poly.iter().map(|p| p.x).min_by(cmp_f).unwrap();
    let max_x = poly.iter().map(|p| p.x).max_by(cmp_f).unwrap();
    let min_y = poly.iter().map(|p| p.y).min_by(cmp_f).unwrap();
    let max_y = poly.iter().map(|p| p.y).max_by(cmp_f).unwrap();

    for x_px in 0..fb.rect.w {
        for y_px in 0..fb.rect.h {

            let p = {
   
                let x_px = x_px as f32;
                let y_px = y_px as f32;
                let w = fb.rect.w as f32;
                let h = fb.rect.h as f32;

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
                fb.set_pixel(x_px, y_px, color);
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
enum Axis { X, Y, Z }
