#![no_std]

extern crate alloc;

use zune_png::PngDecoder;

pub mod content;
pub mod drawing;
pub mod hash;
pub mod input;
pub mod uitk;
pub mod geometry;

use alloc::vec;
use alloc::vec::Vec;
use input::InputState;

#[derive(Debug, Clone)]
#[repr(C)]
pub struct SystemState {
    pub input: InputState,
}

#[derive(Clone, Copy, Hash)]
#[repr(transparent)]
pub struct Color(pub u32);

impl Color {
    pub const WHITE: Color = Color::rgb(255, 255, 255);
    pub const BLACK: Color = Color::rgb(0, 0, 0);
    pub const RED: Color = Color::rgb(255, 0, 0);
    pub const GREEN: Color = Color::rgb(0, 255, 0);
    pub const BLUE: Color = Color::rgb(0, 0, 255);
    pub const YELLOW: Color = Color::rgb(255, 255, 0);
    pub const FUCHSIA: Color = Color::rgb(255, 0, 255);
    pub const AQUA: Color = Color::rgb(0, 250, 255);

    pub const fn rgba(r: u8, g: u8, b: u8, a: u8) -> Self {
        let (r, g, b, a) = (r as u32, g as u32, b as u32, a as u32);

        let val = (a << 3 * 8) + (b << 2 * 8) + (g << 1 * 8) + (r << 0 * 8);

        Color(val)
    }

    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Self::rgba(r, g, b, 255)
    }

    pub const fn from_u32(val: u32) -> Self {
        Color(val)
    }

    pub fn as_rgba(&self) -> (u8, u8, u8, u8) {
        let mask = 0xFFu32;
        let val = self.0;

        let r = ((mask << 0 * 8) & val) >> 0 * 8;
        let g = ((mask << 1 * 8) & val) >> 1 * 8;
        let b = ((mask << 2 * 8) & val) >> 2 * 8;
        let a = ((mask << 3 * 8) & val) >> 3 * 8;

        (r as u8, g as u8, b as u8, a as u8)
    }
}

#[derive(Clone, Debug, PartialEq, Hash)]
pub struct Rect {
    pub x0: i64,
    pub y0: i64,
    pub w: u32,
    pub h: u32,
}

impl Rect {
    pub fn check_contains_point(&self, x: i64, y: i64) -> bool {
        let [x0, y0, x1, y1] = self.as_xyxy();

        return x >= x0 && x <= x1 && y >= y0 && y <= y1;
    }
    pub fn check_contains_rect(&self, other: &Rect) -> bool {
        let [xa0, ya0, xa1, ya1] = self.as_xyxy();
        let [xb0, yb0, xb1, yb1] = other.as_xyxy();

        return xb0 >= xa0 && xb1 <= xa1 && yb0 >= ya0 && yb1 <= ya1;
    }

    pub fn intersection(&self, other: &Rect) -> Option<Rect> {
        let [xa0, ya0, xa1, ya1] = self.as_xyxy();
        let [xb0, yb0, xb1, yb1] = other.as_xyxy();

        if xa1 < xb0 || xb1 < xa0 || ya1 < yb0 || yb1 < ya0 {
            None
        } else {
            let mut x_vals = [xa0, xa1, xb0, xb1];
            x_vals.sort();
            let [_, x0, x1, _] = x_vals;

            let mut y_vals = [ya0, ya1, yb0, yb1];
            y_vals.sort();
            let [_, y0, y1, _] = y_vals;

            Some(Rect {
                x0,
                y0,
                w: (x1 - x0 + 1) as u32,
                h: (y1 - y0 + 1) as u32,
            })
        }
    }

    pub fn bounding_box(&self, other: &Rect) -> Rect {
        let [xa0, ya0, xa1, ya1] = self.as_xyxy();
        let [xb0, yb0, xb1, yb1] = other.as_xyxy();

        let x0 = i64::min(xa0, xb0);
        let y0 = i64::min(ya0, yb0);

        let x1 = i64::max(xa1, xb1);
        let y1 = i64::max(ya1, yb1);

        Rect {
            x0,
            y0,
            w: (x1 - x0 + 1) as u32,
            h: (y1 - y0 + 1) as u32,
        }
    }

    pub fn as_xyxy(&self) -> [i64; 4] {
        let Rect { x0, y0, w, h } = *self;
        let (w, h) = (w as i64, h as i64);
        [x0, y0, x0 + w - 1, y0 + h - 1]
    }

    pub fn from_xyxy(xyxy: [i64; 4]) -> Self {
        let [x0, y0, x1, y1] = xyxy;
        assert!(x0 < x1);
        assert!(y0 < y1);
        let w = (x1 - x0 + 1) as u32;
        let h = (y1 - y0 + 1) as u32;
        Self { x0, y0, w, h }
    }

    pub fn zero_origin(&self) -> Self {
        Rect {
            x0: 0,
            y0: 0,
            w: self.w,
            h: self.h,
        }
    }
}

pub trait FbData {
    fn as_slice(&self) -> &[u32];
}

pub trait FbDataMut: FbData {
    fn as_mut_slice(&mut self) -> &mut [u32];
}

pub struct OwnedPixels(Vec<u32>);
pub struct BorrowedPixels<'a>(&'a [u32]);
pub struct BorrowedMutPixels<'a>(&'a mut [u32]);

impl FbData for OwnedPixels {
    fn as_slice(&self) -> &[u32] {
        self.0.as_slice()
    }
}
impl<'a> FbData for BorrowedPixels<'a> {
    fn as_slice(&self) -> &[u32] {
        self.0
    }
}
impl<'a> FbData for BorrowedMutPixels<'a> {
    fn as_slice(&self) -> &[u32] {
        self.0
    }
}

impl FbDataMut for OwnedPixels {
    fn as_mut_slice(&mut self) -> &mut [u32] {
        self.0.as_mut_slice()
    }
}
impl<'a> FbDataMut for BorrowedMutPixels<'a> {
    fn as_mut_slice(&mut self) -> &mut [u32] {
        self.0
    }
}

pub struct Framebuffer<T> {
    data: T,
    data_w: u32,
    data_h: u32,
    rect: Rect,
}

pub struct FbLine<'a> {
    data: &'a mut [u32],
    x_skipped: u32,
    x_total: u32,
}

impl<'a> FbLine<'a> {
    fn fill(&mut self, color: Color) {
        self.data.fill(color.0)
    }
}

pub trait FbView {
    fn shape(&self) -> (u32, u32);
    fn shape_as_rect(&self) -> Rect;
    fn subregion(&self, rect: &Rect) -> Framebuffer<BorrowedPixels>;
    fn get_pixel(&self, x: i64, y: i64) -> Option<Color>;

    fn to_data_coords(&self, x: i64, y: i64) -> (i64, i64);
    fn get_data(&self) -> &[u32];
    fn get_offset_data_coords(&self, x: i64, y: i64) -> Option<usize>;
    fn get_offset_region_coords(&self, x: i64, y: i64) -> Option<usize>;
}

pub trait FbViewMut: FbView {
    fn subregion_mut(&mut self, rect: &Rect) -> Framebuffer<BorrowedMutPixels>;
    fn set_pixel(&mut self, x: i64, y: i64, color: Color);
    fn fill_line(&mut self, x: i64, line_w: u32, y: i64, color: Color);
    fn fill(&mut self, color: Color);
    fn copy_from_fb<F1: FbView>(&mut self, src: &F1, dst: (i64, i64), blend: bool);

    fn get_data_mut(&mut self) -> &mut [u32];
    fn get_line_mut<'b>(&'b mut self, x: i64, line_w: u32, y: i64) -> FbLine<'b>;
}

impl<'a> Framebuffer<BorrowedMutPixels<'a>> {
    pub fn new<'b>(data: &'b mut [u32], w: u32, h: u32) -> Framebuffer<BorrowedMutPixels<'b>> {
        assert_eq!(data.len(), (w * h) as usize);
        let rect = Rect { x0: 0, y0: 0, w, h };
        Framebuffer {
            data: BorrowedMutPixels(data),
            data_w: w,
            data_h: h,
            rect,
        }
    }
}

impl Framebuffer<OwnedPixels> {
    pub fn new_owned(w: u32, h: u32) -> Self {
        let data = vec![0u32; (w * h) as usize];
        let rect = Rect { x0: 0, y0: 0, w, h };
        Framebuffer {
            data: OwnedPixels(data),
            data_w: w,
            data_h: h,
            rect,
        }
    }

    pub fn from_png(png_bytes: &[u8]) -> Self {
        let mut decoder = PngDecoder::new(png_bytes);
        let decoded = decoder.decode().expect("Invalid PNG bitmap");
        let (w, h) = decoder.get_dimensions().unwrap();

        let data_u8 = decoded.u8().unwrap();

        let data_u32 = unsafe {
            assert_eq!(data_u8.len(), h * w * 4); // Requires an alpha channel
            let mut data_u8 = core::mem::ManuallyDrop::new(data_u8);
            Vec::from_raw_parts(data_u8.as_mut_ptr() as *mut u32, h * w, h * w)
        };

        let (w, h) = (w as u32, h as u32);

        let rect = Rect { x0: 0, y0: 0, w, h };

        Framebuffer {
            data: OwnedPixels(data_u32),
            data_w: w,
            data_h: h,
            rect,
        }
    }
}

impl<T: FbData> FbView for Framebuffer<T> {
    fn shape(&self) -> (u32, u32) {
        (self.rect.w, self.rect.h)
    }

    fn shape_as_rect(&self) -> Rect {
        self.rect.zero_origin()
    }

    fn subregion(&self, rect: &Rect) -> Framebuffer<BorrowedPixels> {
        let Rect { x0, y0, w, h } = *rect;
        let (x0, y0) = self.to_data_coords(x0, y0);

        Framebuffer {
            data: BorrowedPixels(self.data.as_slice()),
            data_w: self.data_w,
            data_h: self.data_h,
            rect: Rect { x0, y0, w, h },
        }
    }

    fn get_offset_region_coords(&self, x: i64, y: i64) -> Option<usize> {
        let (x, y) = self.to_data_coords(x, y);
        self.get_offset_data_coords(x, y)
    }

    fn get_pixel(&self, x: i64, y: i64) -> Option<Color> {
        let data = self.data.as_slice();
        self.get_offset_region_coords(x, y).map(|i| Color(data[i]))
    }

    fn get_data(&self) -> &[u32] {
        self.data.as_slice()
    }

    fn to_data_coords(&self, x: i64, y: i64) -> (i64, i64) {
        let Rect { x0, y0, .. } = self.rect;
        (x + x0, y + y0)
    }

    fn get_offset_data_coords(&self, x: i64, y: i64) -> Option<usize> {
        if x < 0 || y < 0 {
            return None;
        }

        let x = x as u32;
        let y = y as u32;

        if self.data_w <= x || self.data_h <= y {
            return None;
        }

        Some((y * self.data_w + x) as usize)
    }
}

impl<T: FbDataMut> FbViewMut for Framebuffer<T> {
    fn subregion_mut(&mut self, rect: &Rect) -> Framebuffer<BorrowedMutPixels> {
        let Rect { x0, y0, w, h } = *rect;
        let (x0, y0) = self.to_data_coords(x0, y0);

        Framebuffer {
            data: BorrowedMutPixels(self.data.as_mut_slice()),
            data_w: self.data_w,
            data_h: self.data_h,
            rect: Rect { x0, y0, w, h },
        }
    }

    fn set_pixel(&mut self, x: i64, y: i64, color: Color) {
        let offset = self.get_offset_region_coords(x, y);
        let data = self.data.as_mut_slice();
        offset.map(|i| data[i] = color.0);
    }

    fn get_line_mut<'b>(&'b mut self, x: i64, line_w: u32, y: i64) -> FbLine<'b> {
        let (x, y) = self.to_data_coords(x, y);

        if line_w == 0 || y < 0 || y >= self.data_h as i64 {
            return FbLine {
                data: &mut [],
                x_skipped: 0,
                x_total: 0,
            };
        }

        let (x1, x2) = (x, x + line_w as i64 - 1);

        let x1 = i64::max(0, x1);
        let x2 = i64::min(self.data_w as i64, x2);

        let i1 = self.get_offset_data_coords(x1, y).unwrap();
        let i2 = self.get_offset_data_coords(x2, y).unwrap();
        let data = self.data.as_mut_slice();
        let line_slice = &mut data[i1..i2 + 1];

        FbLine {
            data: line_slice,
            x_skipped: (x1 - x) as u32,
            x_total: line_w,
        }
    }

    fn fill_line(&mut self, x: i64, line_w: u32, y: i64, color: Color) {
        self.get_line_mut(x, line_w, y).fill(color);
    }

    fn fill(&mut self, color: Color) {
        let (w, h) = self.shape();
        for y in 0..h {
            self.fill_line(0, w, y.into(), color)
        }
    }

    fn get_data_mut(&mut self) -> &mut [u32] {
        self.data.as_mut_slice()
    }

    fn copy_from_fb<F1: FbView>(&mut self, src: &F1, dst: (i64, i64), blend: bool) {
        let src_rect = src.shape_as_rect();
        let dst_rect = {
            let mut r = self.shape_as_rect();
            let (x, y) = dst;
            r.x0 = x;
            r.y0 = y;
            r
        };

        let (rect_a, rect_b) = {
            let ra = src_rect.intersection(&src.shape_as_rect());
            let rb = dst_rect.intersection(&self.shape_as_rect());

            match (ra, rb) {
                (Some(ra), Some(rb)) => (ra, rb),
                _ => return,
            }
        };

        let w: i64 = u32::min(rect_a.w, rect_b.w).into();
        let h: i64 = u32::min(rect_a.h, rect_b.h).into();

        if w == 0 {
            return;
        }

        for y in 0..h {
            let xa0 = rect_a.x0;
            let xa1 = rect_a.x0 + w - 1;
            let ya = rect_a.y0 + y;

            let ia1 = src.get_offset_region_coords(xa0, ya);
            let ia2 = src.get_offset_region_coords(xa1, ya);

            let (ia1, ia2) = match (ia1, ia2) {
                (Some(ia1), Some(ia2)) => (ia1, ia2),
                _ => continue,
            };

            let xb0 = rect_b.x0;
            let xb1 = rect_b.x0 + w - 1;
            let yb = rect_b.y0 + y;

            let ib1 = self.get_offset_region_coords(xb0, yb);
            let ib2 = self.get_offset_region_coords(xb1, yb);

            let (ib1, ib2) = match (ib1, ib2) {
                (Some(ib1), Some(ib2)) => (ib1, ib2),
                _ => continue,
            };

            let src_data = src.get_data();
            let dst_data = self.data.as_mut_slice();

            if blend {
                dst_data[ib1..=ib2]
                    .iter_mut()
                    .enumerate()
                    .for_each(|(i, v_dst)| {
                        let v_src = Color(src_data[ia1 + i]);
                        *v_dst = blend_colors(v_src, Color(*v_dst)).0;
                    });
            } else {
                dst_data[ib1..=ib2].copy_from_slice(&src_data[ia1..=ia2]);
            }
        }
    }
}

fn blend_colors(c1: Color, c2: Color) -> Color {
    let (r1, g1, b1, a1) = c1.as_rgba();
    let (r2, g2, b2, a2) = c2.as_rgba();

    let r = blend_channel(r2, r1, a1);
    let g = blend_channel(g2, g1, a1);
    let b = blend_channel(b2, b1, a1);

    Color::rgba(r, g, b, a2)
}

fn blend_channel(val_a: u8, val_b: u8, alpha: u8) -> u8 {
    let val_a = val_a as u16;
    let val_b = val_b as u16;
    let alpha = alpha as u16;

    let r = val_a * (256 - alpha) + val_b * (1 + alpha);

    (r >> 8) as u8
}

pub fn decode_png(png_bytes: &[u8]) -> Vec<u8> {
    PngDecoder::new(png_bytes)
        .decode()
        .expect("Invalid PNG bitmap")
        .u8()
        .expect("Invalid PNG bitmap")
}
