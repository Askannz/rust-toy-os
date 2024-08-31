#![no_std]

extern crate alloc;

use core::borrow::BorrowMut;

use zune_png::PngDecoder;

pub mod input;
pub mod drawing;
pub mod uitk;
pub mod hash;
pub mod content;

use alloc::vec::Vec;
use alloc::vec;
use managed::ManagedSlice;
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

        let val =
            (a << 3 * 8) +
            (b << 2 * 8) +
            (g << 1 * 8) +
            (r << 0 * 8);

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
pub struct Rect { pub x0: i64, pub y0: i64, pub w: u32, pub h: u32 }

impl Rect {
    pub fn check_contains_point(&self, x: i64, y: i64) -> bool {

        let [x0, y0, x1, y1] = self.as_xyxy();

        return 
            x >= x0 && x <= x1 &&
            y >= y0 && y <= y1
    }
    pub fn check_contains_rect(&self, other: &Rect) -> bool {

        let [xa0, ya0, xa1, ya1] = self.as_xyxy();
        let [xb0, yb0, xb1, yb1] = other.as_xyxy();

        return 
            xb0 >= xa0 && xb1 <= xa1 &&
            yb0 >= ya0 && yb1 <= ya1
    }

    pub fn intersection(&self, other: &Rect) -> Option<Rect> {

        let [xa0, ya0, xa1, ya1] = self.as_xyxy();
        let [xb0, yb0, xb1, yb1] = other.as_xyxy();

        if (xb0 <= xa1 || xa0 <= xb1) && (yb0 <= ya1 || ya0 <= yb1) {

            let mut x_vals = [xa0, xa1, xb0, xb1];
            x_vals.sort();
            let [_, x0, x1, _] = x_vals;
    
            let mut y_vals = [ya0, ya1, yb0, yb1];
            y_vals.sort();
            let [_, y0, y1, _] = y_vals;

            Some(Rect { x0, y0, w: (x1-x0+1) as u32, h: (y1-y0+1) as u32 })
        } else {
            None
        }
    }

    pub fn bounding_box(&self, other: &Rect) -> Rect {

        let [xa0, ya0, xa1, ya1] = self.as_xyxy();
        let [xb0, yb0, xb1, yb1] = other.as_xyxy();

        let x0 = i64::min(xa0, xb0);
        let y0 = i64::min(ya0, yb0);

        let x1 = i64::max(xa1, xb1);
        let y1 = i64::max(ya1, yb1);

        Rect { x0, y0, w: (x1-x0+1) as u32, h: (y1-y0+1) as u32 }
    }

    pub fn as_xyxy(&self) -> [i64; 4] {
        let Rect { x0, y0, w, h } = *self;
        let (w, h) = (w as i64, h as i64);
        [x0, y0, x0+w-1, y0+h-1]
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
        Rect { x0: 0, y0: 0, w: self.w, h: self.h }
    }
}

pub struct Framebuffer<'a> {
    data: ManagedSlice<'a, u32>,
    data_w: u32,
    data_h: u32,
    rect: Rect,
}

impl<'a> Framebuffer<'a> {

    pub fn new_owned(w: u32, h: u32) -> Self {
        let data = vec![0u32; (w * h) as usize];
        let rect = Rect { x0: 0, y0: 0, w, h };
        Framebuffer { data: ManagedSlice::Owned(data), data_w: w , data_h: h, rect }
    }

    pub fn new(data: &'a mut [u32], w: u32, h: u32) -> Self {
        assert_eq!(data.len(), (w * h) as usize);
        let rect = Rect { x0: 0, y0: 0, w, h };
        Framebuffer { data: ManagedSlice::Borrowed(data), data_w: w , data_h: h, rect }
    }

    pub fn from_png(png_bytes: &[u8]) -> Self {

        let mut decoder =  PngDecoder::new(png_bytes);
        let decoded = decoder.decode().expect("Invalid PNG bitmap");
        let (w, h) = decoder.get_dimensions().unwrap();

        let data_u8 = decoded.u8().unwrap();

        let data_u32 = unsafe {
            assert_eq!(data_u8.len(), h * w * 4); // Requires an alpha channel
            let mut data_u8 = core::mem::ManuallyDrop::new(data_u8);
            Vec::from_raw_parts(
                data_u8.as_mut_ptr() as *mut u32,
                h * w,
                h * w
            )
        };

        let (w, h) = (w as u32, h as u32);

        let rect = Rect { x0: 0, y0: 0, w, h };
    
        Framebuffer {
            data: ManagedSlice::Owned(data_u32),
            data_w: w,
            data_h: h,
            rect,
        }
    }

    pub fn shape(&self) -> (u32, u32) {
        (self.rect.w, self.rect.h)
    }

    pub fn shape_as_rect(&self) -> Rect {
        self.rect.zero_origin()
    }

    pub fn subregion<'b>(&'b mut self, rect: &Rect) -> Framebuffer<'b> {

        let Rect { x0, y0, w, h } = *rect;
        let (x0, y0) = self.to_data_coords(x0, y0);

        Framebuffer {
            data: ManagedSlice::Borrowed(self.data.borrow_mut()),
            data_w: self.data_w,
            data_h: self.data_h,
            rect: Rect { x0, y0, w, h }
        }

    }

    fn to_data_coords(&self, x: i64, y: i64) -> (i64, i64) {
        let Rect { x0, y0, .. } = self.rect;
        (x+x0, y+y0)
    }

    fn get_offset(&self, x: i64, y: i64) -> Option<usize> {

        let (x, y) = self.to_data_coords(x, y);

        match self.rect.check_contains_point(x, y) {
            true => Some((y as u32 * self.data_w + x as u32) as usize),
            false => None,
        }
    }

    pub fn get_pixel(&self, x: i64, y: i64) -> Option<Color> {
        self.get_offset(x, y).map(|i| Color(self.data[i]))
    }

    pub fn set_pixel(&mut self, x: i64, y: i64, color: Color) {
        self.get_offset(x, y).map(|i| self.data[i] = color.0);
    }

    pub fn fill_line(&mut self, x: i64, line_w: u32, y: i64, color: Color) {

        let (w, h): (i64, i64) = (self.rect.w.into(), self.rect.h.into());
        let line_w: i64 = line_w.into();

        if y < 0 || y >= h || line_w == 0 { return }

        let x1 = i64::max(x, 0);
        let x2 = i64::min(x+line_w-1, w-1);

        let i1 = self.get_offset(x1, y).unwrap();
        let i2 = self.get_offset(x2, y).unwrap();
        self.data[i1..=i2].fill(color.0);
    }

    pub fn copy_from_fb(&mut self, src: &Framebuffer, src_rect: &Rect, dst_rect: &Rect, blend: bool) {

        let (rect_a, rect_b) =  {

            let ra = src_rect.intersection(&src.shape_as_rect());
            let rb = dst_rect.intersection(&self.shape_as_rect());

            match (ra, rb) {
                (Some(ra), Some(rb)) => (ra, rb),
                _ => return,
            }
        };

        let w: i64 = u32::min(rect_a.w, rect_b.w).into();
        let h: i64 = u32::min(rect_a.h, rect_b.h).into();

        if w == 0 { return; }

        for y in 0..h {

            let xa0 = rect_a.x0;
            let xa1 = rect_a.x0 + w - 1;
            let ya = rect_a.y0 + y;

            let ia1 = src.get_offset(xa0, ya);
            let ia2 = src.get_offset(xa1, ya);

            let (ia1, ia2) = match (ia1, ia2) {
                (Some(ia1), Some(ia2)) => (ia1, ia2),
                _ => continue,
            };

            let xb0 = rect_b.x0;
            let xb1 = rect_b.x0 + w - 1;
            let yb = rect_b.y0 + y;

            let ib1 = self.get_offset(xb0, yb);
            let ib2 = self.get_offset(xb1, yb);

            let (ib1, ib2) = match (ib1, ib2) {
                (Some(ib1), Some(ib2)) => (ib1, ib2),
                _ => continue,
            };

            if blend {
                self.data[ib1..=ib2].iter_mut()
                    .enumerate()
                    .for_each(|(i, v_dst)| {
                        let v_src = Color(src.data[ia1+i]);
                        *v_dst = blend_colors(v_src, Color(*v_dst)).0;
                    });
            } else {
                self.data[ib1..=ib2].copy_from_slice(&src.data[ia1..=ia2]);
            }
        }
    }

    pub fn fill(&mut self, color: Color) {
        self.data.fill(color.0);
    }
}

fn blend_colors(c1: Color, c2: Color) -> Color{

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
        .decode().expect("Invalid PNG bitmap")
        .u8().expect("Invalid PNG bitmap")
}
