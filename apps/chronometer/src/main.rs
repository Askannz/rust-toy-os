#![no_std]
#![no_main]

use core::arch::asm;
use core::panic::PanicInfo;
use core::fmt::Write;
use uart_16550::SerialPort;

use core::cmp::Ordering;
use micromath::F32Ext;




// ---- SHARED ----

#[derive(Clone)]
struct Color(u8, u8, u8);
#[derive(Clone)]
struct Rect { x0: i32, y0: i32, w: i32, h: i32 }

impl Rect {
    fn check_in(&self, x: i32, y: i32) -> bool {
        return 
            x >= self.x0 && x < self.x0 + self.w &&
            y >= self.y0 && y < self.y0 + self.h
    }
}

pub struct Framebuffer<'a> {
    data: &'a mut [u8],
    w: i32,
    h: i32,
}

pub struct FrameBufSlice<'a, 'b> {
    fb: &'b mut Framebuffer<'a>,
    rect: Rect
}

impl<'a, 'b> FrameBufSlice<'a, 'b> {
    fn set_pixel(&mut self, x: i32, y: i32, color: &Color) {
        let Color(r, g, b) = *color;
        let i = (((y+self.rect.y0) * self.fb.w + x + self.rect.x0) * 4) as usize;
        self.fb.data[i] = r;
        self.fb.data[i+1] = g;
        self.fb.data[i+2] = b;
        self.fb.data[i+3] = 0xff;
    }
}

#[repr(C)]
pub struct Oshandle<'a, 'b> {
    fb: FrameBufSlice<'a, 'b>,
    cursor_x: i32,
    cursor_y: i32,
}

// ---- END SHARED ----

const COLORS: [Color; 6] = [
    Color(0xff, 0x00, 0x00),
    Color(0x00, 0xff, 0x00),
    Color(0x00, 0x00, 0xff),
    Color(0xff, 0xff, 0x00),
    Color(0xff, 0x00, 0xff),
    Color(0x00, 0xff, 0xff),
];
const ZOOM: f32 = 0.2;
const PI: f32 = 3.14159265359;


#[no_mangle]
pub extern "C" fn efi_main(handle: &mut Oshandle) {

    draw_chrono(&mut handle.fb);
}


/// This function is called on panic.
#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {}
}

pub fn draw_chrono(fb: &mut FrameBufSlice) {

    const N_CYCLES: u64 = 30000000000;
    let timestamp = unsafe { core::arch::x86_64::_rdtsc()};
    let angle = (((timestamp % N_CYCLES) as f32) / (N_CYCLES as f32)) * 2.0 * PI;

    let quad = [
        Point { x: -0.5, y: 0.0, z: 0.0 },
        Point { x: 0.5, y: 0.0, z: 0.0 },
        Point { x: 0.1, y: 4.0, z: 0.0 },
        Point { x: -0.1, y: 4.0, z: 0.0 }
    ];

    let quad = rotate(&quad, Axis::Z, angle);

    rasterize_poly(fb, &quad, &Color(0xff, 0xff, 0xff));
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
 