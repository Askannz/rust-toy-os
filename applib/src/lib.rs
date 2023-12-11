#![no_std]

extern crate alloc;

use zune_png::PngDecoder;

pub mod keymap;
pub mod drawing;

use alloc::vec::Vec;
use keymap::Keycode;

pub const MAX_KEYS_PRESSED: usize = 3;
pub type KeyboardState = [Option<Keycode>; MAX_KEYS_PRESSED];

#[derive(Debug, Clone)]
#[repr(C)]
pub struct SystemState {
    pub pointer: PointerState,
    pub keyboard: KeyboardState,
    pub time: f64,
}

#[derive(Debug, Clone)]
#[repr(C)]
pub struct PointerState {
    pub x: i64,
    pub y: i64,
    pub left_clicked: bool,
    pub right_clicked: bool
}

#[derive(Clone, Copy)]
#[repr(transparent)]
pub struct Color(pub u32);

impl Color {

    pub const fn from_rgba(r: u8, g: u8, b: u8, a: u8) -> Self {

        let (r, g, b, a) = (r as u32, g as u32, b as u32, a as u32);

        let val =
            (a << 3 * 8) +
            (b << 2 * 8) +
            (g << 1 * 8) +
            (r << 0 * 8);

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

#[derive(Clone, Debug)]
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

        let x0 = i64::max(xa0, xb0);
        let y0 = i64::max(ya0, yb0);

        let x1 = i64::min(xa1, xb1);
        let y1 = i64::min(ya1, yb1);

        if x0 <= x1 && y0 <= y1 {
            Some(Rect { x0, y0, w: (x1-x0+1) as u32, h: (y1-y0+1) as u32 })
        } else {
            None
        }
    }

    pub fn as_xyxy(&self) -> [i64; 4] {
        let Rect { x0, y0, w, h } = *self;
        let (w, h) = (w as i64, h as i64);
        [x0, y0, x0+w-1, y0+h-1]
    }
}

pub struct Framebuffer<'a> {
    pub data: &'a mut [u32],
    pub w: usize,
    pub h: usize,
    pub rect: Rect,
}

impl<'a> Framebuffer<'a> {
    pub fn new(data: &'a mut [u32], w: usize, h: usize) -> Self {
        assert_eq!(data.len(), w * h);
        let rect = Rect { x0: 0, y0: 0, w: w as u32, h: h as u32 };
        Framebuffer { data, w, h, rect }
    }

}

impl<'a> Framebuffer<'a> {

    pub fn get_region(&mut self, rect: &Rect) -> Option<Framebuffer> {

        let clipped = self.clip_rect(rect)?;

        let new_view = Rect { 
            x0: self.rect.x0 + clipped.x0,
            y0: self.rect.y0 + clipped.y0,
            w: clipped.w,
            h: clipped.h,
        };

        let [x0, y0, x1, y1] = new_view.as_xyxy();
        assert!(x0 >= 0 && x0 < self.w as i64 && y0 >= 0 && y0 < self.h as i64);

        let Framebuffer { w, h, .. } = *self;
        Some(Framebuffer {  data: self.data, w, h, rect: new_view })
    }

    pub fn get_offset(&self, x: u32, y: u32) -> usize {
        let Rect { x0, y0, .. } = self.rect;
        let (x0, y0) = (x0 as u32, y0 as u32);
        (y0 + y) as usize * self.w + (x0 + x) as usize
    }

    pub fn get_pixel(&self, x: u32, y: u32) -> Color {
        let i = self.get_offset(x, y);
        Color(self.data[i])
    }

    pub fn set_pixel(&mut self, x: u32, y: u32, color: Color) {
        let i = self.get_offset(x, y);
        self.data[i] = color.0;
    }

    pub fn fill_line(&mut self, x: u32, w: u32, y: u32, color: Color) {
        let i1 = self.get_offset(x, y);
        let i2 = self.get_offset(x+w, y);
        self.data[i1..i2].fill(color.0);
    }

    pub fn blend(&mut self, other: &Framebuffer) {

        let w = u32::min(self.rect.w, other.rect.w);
        let h = u32::min(self.rect.h, other.rect.h);

        for x in 0..w {
            for y in 0..h {
                let px_1 = self.get_pixel(x, y);
                let px_2 = other.get_pixel(x, y);
                let blended = blend_colors(px_2, px_1);
                self.set_pixel(x, y, blended);
            }
        }
    }

    pub fn clip_rect(&self, rect: &Rect) -> Option<Rect> {
        let view_rect = Rect { x0: 0, y0: 0, w: self.rect.w, h: self.rect.h };
        view_rect.intersection(rect)
    }

    pub fn copy_from(&mut self, other: &Framebuffer) {
        let w = u32::min(self.rect.w, other.rect.w);
        let h = u32::min(self.rect.h, other.rect.h);

        for y in 0..h {
            let ia1 = self.get_offset(0, y);
            let ia2 = self.get_offset(w-1, y);
            let ib1 = other.get_offset(0, y);
            let ib2 = other.get_offset(w-1, y);
            self.data[ia1..=ia2].copy_from_slice(&other.data[ib1..=ib2]);
        }
    }

    pub fn fill(&mut self, color: Color) {
    
        let Rect { x0, y0, w, h } = self.rect;
    
        let (x0, y0) = (x0 as u32, y0 as u32);

        for y in y0..y0+h {
            self.fill_line(x0, w, y, color)
        }
    }
}

fn blend_colors(c1: Color, c2: Color) -> Color{

    let (r1, g1, b1, a1) = c1.as_rgba();
    let (r2, g2, b2, a2) = c2.as_rgba();

    let r = blend_channel(r2, r1, a1);
    let g = blend_channel(g2, g1, a1);
    let b = blend_channel(b2, b1, a1);
    
    Color::from_rgba(r, g, b, a2)
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