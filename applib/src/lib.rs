#![no_std]

extern crate alloc;

use zune_png::PngDecoder;

pub mod content;
pub mod drawing;
pub mod geometry;
pub mod hash;
pub mod input;
pub mod uitk;
mod stylesheet;

use alloc::vec;
use alloc::vec::Vec;
use core::ops;
use geometry::Vec2D;
use input::InputState;

pub use stylesheet::{StyleSheet, StyleSheetColors};

#[derive(Clone, Copy, Hash)]
#[repr(transparent)]
pub struct Color(pub [u8; 4]);

impl Color {
    pub const ZERO: Color = Color::rgba(0, 0, 0, 0);
    pub const WHITE: Color = Color::rgb(255, 255, 255);
    pub const BLACK: Color = Color::rgb(0, 0, 0);
    pub const RED: Color = Color::rgb(255, 0, 0);
    pub const GREEN: Color = Color::rgb(0, 255, 0);
    pub const BLUE: Color = Color::rgb(0, 0, 255);
    pub const YELLOW: Color = Color::rgb(255, 255, 0);
    pub const FUCHSIA: Color = Color::rgb(255, 0, 255);
    pub const AQUA: Color = Color::rgb(0, 250, 255);

    pub const fn rgba(r: u8, g: u8, b: u8, a: u8) -> Self {
        Color([r, g, b, a])
    }

    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Self::rgba(r, g, b, 255)
    }

    pub const fn from_u32(val: &[u8; 4]) -> Self {
        Color(*val)
    }

    pub fn as_rgba(&self) -> (u8, u8, u8, u8) {
        let Color([r, g, b, a]) = *self;
        (r, g, b, a)
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
    // TODO: use Point2D everywhere

    pub fn origin(&self) -> (i64, i64) {
        (self.x0, self.y0)
    }

    pub fn center(&self) -> (i64, i64) {
        (self.x0 + (self.w / 2) as i64, self.y0 + (self.h / 2) as i64)
    }

    pub fn from_center(xc: i64, yc: i64, w: u32, h: u32) -> Self {
        Self {
            x0: xc - (w / 2) as i64,
            y0: yc - (h / 2) as i64,
            w,
            h,
        }
    }

    pub fn align_to_rect(&self, other: &Rect) -> Rect {
        let (x1c, y1c) = other.center();
        Self::from_center(x1c, y1c, self.w, self.h)
    }

    pub fn align_to_rect_vert(&self, other: &Rect) -> Rect {
        let (x0c, _) = self.center();
        let (_, y1c) = other.center();
        Self::from_center(x0c, y1c, self.w, self.h)
    }

    pub fn align_to_rect_horiz(&self, other: &Rect) -> Rect {
        let (_, y0c) = self.center();
        let (x1c, _) = other.center();
        Self::from_center(x1c, y0c, self.w, self.h)
    }

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
        assert!(x0 <= x1);
        assert!(y0 <= y1);
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

impl ops::Add<Vec2D<i64>> for Rect {
    type Output = Rect;
    fn add(self, vec: Vec2D<i64>) -> Self::Output {
        Self::Output {
            x0: self.x0 + vec.x,
            y0: self.y0 + vec.y,
            w: self.w,
            h: self.h,
        }
    }
}

pub trait FbData {
    fn as_slice(&self) -> &[Color];
}

pub trait FbDataMut: FbData {
    fn as_mut_slice(&mut self) -> &mut [Color];
}

pub struct OwnedPixels(Vec<Color>);
pub struct BorrowedPixels<'a>(&'a [Color]);
pub struct BorrowedMutPixels<'a>(&'a mut [Color]);

impl FbData for OwnedPixels {
    fn as_slice(&self) -> &[Color] {
        self.0.as_slice()
    }
}
impl<'a> FbData for BorrowedPixels<'a> {
    fn as_slice(&self) -> &[Color] {
        self.0
    }
}
impl<'a> FbData for BorrowedMutPixels<'a> {
    fn as_slice(&self) -> &[Color] {
        self.0
    }
}

impl FbDataMut for OwnedPixels {
    fn as_mut_slice(&mut self) -> &mut [Color] {
        self.0.as_mut_slice()
    }
}
impl<'a> FbDataMut for BorrowedMutPixels<'a> {
    fn as_mut_slice(&mut self) -> &mut [Color] {
        self.0
    }
}

pub struct Framebuffer<T> {
    data: T,
    data_w: u32,
    data_h: u32,
    rect: Rect,
}

pub struct FbLineMut<'a> {
    data: &'a mut [Color],
    x_data_start: u32,
    line_w: u32,
}

pub struct FbLine<'a> {
    data: &'a [Color],
    x_data_start: u32,
    line_w: u32,
}

pub struct FbLineCoords {
    data_index: Option<usize>,
    data_len: usize,
    x_data_start: u32,
}

impl<'a> FbLineMut<'a> {
    fn fill(&mut self, color: Color, blend: bool) {
        if blend {
            self.data.iter_mut().for_each(|pixel| {
                *pixel = blend_colors(color, *pixel);
            });
        } else {
            self.data.fill(color)
        }
    }

    fn copy_from_line(&mut self, other: &FbLine, blend: bool) {
        assert_eq!(self.line_w, other.line_w);

        let new_x_data_start = u32::max(self.x_data_start, other.x_data_start);
        let new_x_data_end = u32::min(
            self.x_data_start + self.data.len() as u32,
            other.x_data_start + other.data.len() as u32,
        );

        let copy_len = (new_x_data_end - new_x_data_start) as usize;

        let i1 = (new_x_data_start - self.x_data_start) as usize;
        let i2 = (new_x_data_start - other.x_data_start) as usize;

        if blend {
            self.data[i1..i1 + copy_len]
                .iter_mut()
                .zip(other.data[i2..i2 + copy_len].iter())
                .for_each(|(dst, src)| {
                    *dst = blend_colors(*src, *dst);
                });
        } else {
            self.data[i1..i1 + copy_len].copy_from_slice(&other.data[i2..i2 + copy_len]);
        }
    }
}

pub trait FbView {
    fn shape(&self) -> (u32, u32);
    fn shape_as_rect(&self) -> Rect;
    fn subregion(&self, rect: &Rect) -> Framebuffer<BorrowedPixels>;
    fn get_pixel(&self, x: i64, y: i64) -> Option<Color>;

    fn to_data_coords(&self, x: i64, y: i64) -> (i64, i64);
    fn get_data(&self) -> &[Color];
    fn get_offset_data_coords(&self, x: i64, y: i64) -> Option<usize>;
    fn get_offset_region_coords(&self, x: i64, y: i64) -> Option<usize>;

    fn get_line_coords(&self, x: i64, line_w: u32, y: i64) -> FbLineCoords;
    fn get_line<'b>(&'b self, x: i64, line_w: u32, y: i64) -> FbLine<'b>;
}

pub trait FbViewMut: FbView {
    fn subregion_mut(&mut self, rect: &Rect) -> Framebuffer<BorrowedMutPixels>;
    fn set_pixel(&mut self, x: i64, y: i64, color: Color);
    fn fill_line(&mut self, x: i64, line_w: u32, y: i64, color: Color, blend: bool);
    fn fill(&mut self, color: Color);
    fn copy_from_fb<F1: FbView>(&mut self, src: &F1, dst: (i64, i64), blend: bool);
    fn get_data_mut(&mut self) -> &mut [Color];
    fn get_line_mut<'b>(&'b mut self, x: i64, line_w: u32, y: i64) -> FbLineMut<'b>;
}

impl<'a> Framebuffer<BorrowedMutPixels<'a>> {

    pub fn new<'b>(data: &'b mut [Color], w: u32, h: u32) -> Framebuffer<BorrowedMutPixels<'b>> {
        assert_eq!(data.len(), (w * h) as usize);
        let rect = Rect { x0: 0, y0: 0, w, h };
        Framebuffer {
            data: BorrowedMutPixels(data),
            data_w: w,
            data_h: h,
            rect,
        }
    }

    pub fn from_bytes<'b>(bytes: &'b mut [u8], w: u32, h: u32) -> Framebuffer<BorrowedMutPixels<'b>> {

        assert_eq!(bytes.len(), (4 * w * h) as usize);

        // Safe because of the check above, and because Color has an alignment of 1
        let data = unsafe { 
            let (head, body, tail) = bytes.align_to_mut::<Color>();
            assert_eq!(head.len(), 0);
            assert_eq!(tail.len(), 0);
            body
        };

        Self::new(data, w, h)
    }
}

impl<'a> Framebuffer<BorrowedPixels<'a>> {

    pub fn new<'b>(data: &'b [Color], w: u32, h: u32) -> Framebuffer<BorrowedPixels<'b>> {
        assert_eq!(data.len(), (w * h) as usize);
        let rect = Rect { x0: 0, y0: 0, w, h };
        Framebuffer {
            data: BorrowedPixels(data),
            data_w: w,
            data_h: h,
            rect,
        }
    }

    pub fn from_bytes<'b>(bytes: &'b [u8], w: u32, h: u32) -> Framebuffer<BorrowedPixels<'b>> {

        assert_eq!(bytes.len(), (4 * w * h) as usize);

        // Safe because of the check above, and because Color has an alignment of 1
        let data = unsafe { 
            let (head, body, tail) = bytes.align_to::<Color>();
            assert_eq!(head.len(), 0);
            assert_eq!(tail.len(), 0);
            body
        };

        Self::new(data, w, h)
    }
}

impl Framebuffer<OwnedPixels> {
    pub fn new_owned(w: u32, h: u32) -> Self {
        let data = vec![Color([0u8; 4]); (w * h) as usize];
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
        assert_eq!(data_u8.len(), h * w * 4, "PNG has wrong dimensions"); // Requires an alpha channel

        let data: Vec<Color> = (0..h*w).map(|x| {
            let i = 4 * x;
            let color_bytes = data_u8[i..i+4].try_into().unwrap();
            Color(color_bytes)
        })
        .collect();

        let (w, h) = (w as u32, h as u32);

        let rect = Rect { x0: 0, y0: 0, w, h };

        Framebuffer {
            data: OwnedPixels(data),
            data_w: w,
            data_h: h,
            rect,
        }
    }

    pub fn size_bytes(&self) -> usize {
        self.data.as_slice().len() * 4
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
        self.get_offset_region_coords(x, y).map(|i| data[i])
    }

    fn get_data(&self) -> &[Color] {
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

    fn get_line_coords(&self, x: i64, line_w: u32, y: i64) -> FbLineCoords {
        let (x, y) = self.to_data_coords(x, y);

        let empty_line = FbLineCoords {
            data_index: None,
            data_len: 0,
            x_data_start: line_w,
        };

        if line_w == 0 || y < 0 || y >= self.data_h as i64 {
            return empty_line;
        }

        let (x1, x2) = (x, x + line_w as i64 - 1);

        // Clipping to subregion rect
        let x1 = i64::max(self.rect.x0, x1);
        let x2 = i64::min(self.rect.x0 + self.rect.w as i64 - 1, x2);

        // Clipping to data bounds
        let x1 = i64::max(0, x1);
        let x2 = i64::min(self.data_w as i64 - 1, x2);

        let i1 = self.get_offset_data_coords(x1, y);
        let i2 = self.get_offset_data_coords(x2, y);

        let (i1, i2) = match (i1, i2) {
            (Some(i1), Some(i2)) => (i1, i2),
            _ => return empty_line,
        };

        FbLineCoords {
            data_index: Some(i1),
            data_len: (i2 - i1 + 1) as usize,
            x_data_start: (x1 - x) as u32,
        }
    }

    fn get_line<'b>(&'b self, x: i64, line_w: u32, y: i64) -> FbLine<'b> {
        let FbLineCoords {
            data_index,
            data_len,
            x_data_start,
            ..
        } = self.get_line_coords(x, line_w, y);

        let data = self.data.as_slice();

        let line_slice = match data_index {
            Some(i) => &data[i..i + data_len],
            None => &[],
        };

        FbLine {
            data: line_slice,
            x_data_start,
            line_w,
        }
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
        offset.map(|i| data[i] = color);
    }

    fn fill_line(&mut self, x: i64, line_w: u32, y: i64, color: Color, blend: bool) {
        self.get_line_mut(x, line_w, y).fill(color, blend);
    }

    fn fill(&mut self, color: Color) {

        let (w, h) = self.shape();
        if w == self.data_w && h == self.data_h {
            self.data.as_mut_slice().fill(color);
        } else {
            for y in 0..h {
                self.fill_line(0, w, y.into(), color, false)
            }
        }
    }

    fn get_data_mut(&mut self) -> &mut [Color] {
        self.data.as_mut_slice()
    }

    fn get_line_mut<'b>(&'b mut self, x: i64, line_w: u32, y: i64) -> FbLineMut<'b> {
        let FbLineCoords {
            data_index,
            data_len,
            x_data_start,
            ..
        } = self.get_line_coords(x, line_w, y);

        let data = self.data.as_mut_slice();

        let line_slice = match data_index {
            Some(i) => &mut data[i..i + data_len],
            None => &mut [],
        };

        FbLineMut {
            data: line_slice,
            x_data_start,
            line_w,
        }
    }

    fn copy_from_fb<F1: FbView>(&mut self, src: &F1, dst: (i64, i64), blend: bool) {
        let (x0, y0) = dst;
        let (src_w, src_h) = src.shape();

        for y in 0..(src_h as i64) {
            let src_line = src.get_line(0, src_w, y);
            let mut dst_line = self.get_line_mut(x0, src_line.line_w, y0 + y);
            dst_line.copy_from_line(&src_line, blend);
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
