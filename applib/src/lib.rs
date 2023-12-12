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
    pub w: u32,
    pub h: u32,
}

impl<'a> Framebuffer<'a> {
    pub fn new(data: &'a mut [u32], w: u32, h: u32) -> Self {
        assert_eq!(data.len(), (w * h) as usize);
        Framebuffer { data, w, h }
    }

}

impl<'a> Framebuffer<'a> {

    fn check_valid_point(&self, x: i64, y: i64) -> bool {
        let (w, h): (i64, i64) = (self.w.into(), self.h.into());
        return 0 <= x && x < w && 0 <= y && y < h;
    }

    fn get_offset(&self, x: i64, y: i64) -> Option<usize> {
        if self.check_valid_point(x, y) {
            Some((y as u32 * self.w + x as u32) as usize)
        } else {
            None
        }
        
    }

    pub fn get_pixel(&self, x: i64, y: i64) -> Option<Color> {
        self.get_offset(x, y).map(|i| Color(self.data[i]))
    }

    pub fn set_pixel(&mut self, x: i64, y: i64, color: Color) {
        self.get_offset(x, y).map(|i| self.data[i] = color.0);
    }

    pub fn fill_line(&mut self, x: i64, line_w: u32, y: i64, color: Color) {

        let (w, h): (i64, i64) = (self.w.into(), self.h.into());
        let line_w: i64 = line_w.into();

        if y < 0 || y >= h || line_w == 0 { return }

        let x1 = i64::max(x, 0);
        let x2 = i64::min(x+line_w-1, w-1);

        let i1 = self.get_offset(x1, y).unwrap();
        let i2 = self.get_offset(x2, y).unwrap();
        self.data[i1..=i2].fill(color.0);
    }

    pub fn copy_fb(&mut self, src: &Framebuffer, rect: &Rect) {

        let Rect { x0: x, y0: y, w: w_rect, h: h_rect } = *rect;

        let (wa, ha): (i64, i64) = (self.w.into(), self.h.into());
        let (wb, hb): (i64, i64) = (src.w.into(), src.h.into());
        let (w_rect, h_rect): (i64, i64) = (w_rect.into(), h_rect.into());

        let w_copy = i64::min(wb, w_rect);
        let h_copy = i64::min(hb, h_rect);

        if wb == 0 { return; }

        let ya1 = i64::max(y, 0);
        let ya2 = i64::min(y + h_copy - 1, ha - 1);

        for ya in ya1..=ya2 {
    
            let yb = ya - y;

            let xa1 = i64::max(x, 0);
            let xa2 = i64::min(x + w_copy - 1, wa - 1);

            let xb1 = xa1 - x;
            let xb2 = xa2 - x;

            let ia1 = self.get_offset(xa1, ya).unwrap();
            let ia2 = self.get_offset(xa2, ya).unwrap();

            let ib1 = src.get_offset(xb1, yb).unwrap();
            let ib2 = src.get_offset(xb2, yb).unwrap();

            self.data[ia1..=ia2].copy_from_slice(&src.data[ib1..=ib2]);
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