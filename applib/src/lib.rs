#![no_std]

extern crate alloc;

pub mod keymap;
pub mod drawing;

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
    pub x: u32,
    pub y: u32,
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
pub struct Rect { pub x0: u32, pub y0: u32, pub w: u32, pub h: u32 }

impl Rect {
    pub fn check_contains_point(&self, x: u32, y: u32) -> bool {
        return 
            x >= self.x0 && x < self.x0 + self.w &&
            y >= self.y0 && y < self.y0 + self.h
    }
    pub fn check_contains_rect(&self, other: &Rect) -> bool {
        return 
            other.x0 >= self.x0 && other.x0 + other.w <= self.x0 + self.w &&
            other.y0 >= self.y0 && other.y0 + other.h <= self.y0 + self.h
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

    pub fn get_region(&mut self, rect: &Rect) -> Framebuffer {
        assert!(self.rect.check_contains_rect(rect));
        let Framebuffer { w, h, .. } = *self;
        Framebuffer {  data: self.data, w, h, rect: rect.clone() }
    }

    pub fn get_offset(&self, x: u32, y: u32) -> usize {
        let Rect { x0, y0, .. } = self.rect;
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