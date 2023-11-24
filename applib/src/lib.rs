#![no_std]

extern crate alloc;

pub mod keymap;

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

#[derive(Clone)]
pub struct Color(pub u8, pub u8, pub u8);
#[derive(Clone)]
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
    pub data: &'a mut [u8],
    pub w: usize,
    pub h: usize,
    pub rect: Rect,
}

impl<'a> Framebuffer<'a> {
    pub fn new(data: &'a mut [u8], w: usize, h: usize) -> Self {
        assert_eq!(data.len(), w * h * 4);
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
        assert!(x < self.rect.w && y < self.rect.h);
        let Rect { x0, y0, .. } = self.rect;
        ((y0 + y) as usize * self.w + (x0 + x) as usize) * 4
    }

    pub fn get_pixel_mut(&mut self, x: u32, y: u32) -> &mut [u8] {
        let i = self.get_offset(x, y);
        &mut self.data[i..i+4]
    }

    pub fn get_pixel(&self, x: u32, y: u32) -> &[u8] {
        let i = self.get_offset(x, y);
        &self.data[i..i+4]
    }

    pub fn set_pixel(&mut self, x: u32, y: u32, color: &Color) {
        let Color(r, g, b) = *color;
        let i = self.get_offset(x, y);
        self.data[i] = r;
        self.data[i+1] = g;
        self.data[i+2] = b;
        self.data[i+3] = 0xFF;
    }

    pub fn blend(&mut self, other: &Framebuffer) {

        let w = u32::min(self.rect.w, other.rect.w);
        let h = u32::min(self.rect.h, other.rect.h);

        for x in 0..w {
            for y in 0..h {
                let px_1 = self.get_pixel_mut(x, y);
                let px_2 = other.get_pixel(x, y);
                let alpha = px_2[3];
                px_1[0] = blend(px_1[0], px_2[0], alpha);
                px_1[1] = blend(px_1[1], px_2[1], alpha);
                px_1[2] = blend(px_1[2], px_2[2], alpha);
            }
        }
    }

    pub fn fill(&mut self, value: &[u8]) {

        assert_eq!(value.len(), 4);
    
        let Rect { x0, y0, w, h } = self.rect;
    
        for x in x0..x0+w {
            for y in y0..y0+h {
                self.get_pixel_mut(x, y).copy_from_slice(value);
            }
        }
    }
}

pub struct Font {
    pub fontmap: &'static [u8],
    pub nb_chars: usize,
    pub char_h: usize,
    pub char_w: usize,
}

pub const DEFAULT_FONT: Font = Font {
    fontmap: include_bytes!("../fontmap.bin"),
    nb_chars: 95,
    char_h: 24,
    char_w: 12,
};

pub fn draw_text_rect(fb: &mut Framebuffer, s: &str, rect: &Rect, font: &Font, color: &Color) {
    
    let Rect { x0, y0, w, h } = *rect;
    let char_h = font.char_h;

    let max_per_line = w as usize / font.char_w;

    let mut i0 = 0;
    let mut y = y0;
    for (i, c) in s.chars().enumerate() {

        let i1 = {
            if c == '\n' { Some(i) }
            else if i - i0 + 1 >= max_per_line || i == s.len() - 1 { Some(i+1) }
            else { None }
        };

        if let Some(i1) = i1 {
            draw_str(fb, &s[i0..i1], x0, y, font, color);
            i0 = i + 1;
            y += char_h as u32;
        }
    }
}

pub fn draw_str(fb: &mut Framebuffer, s: &str, x0: u32, y0: u32, font: &Font, color: &Color) {
    let mut x = x0;
    for c in s.as_bytes() {
        draw_char(fb, *c, x, y0, font, color);
        x += font.char_w as u32;
    }
}

fn draw_char(fb: &mut Framebuffer, mut c: u8, x0: u32, y0: u32, font: &Font, color: &Color) {

    // Replacing unsupported chars with spaces
    if c < 32 || c > 126 { c = 32}

    let c_index = (c - 32) as usize;
    let Color(r, g, b) = *color;
    let Font { nb_chars, char_h, char_w, .. } = *font;

    for x in 0..char_w {
        for y in 0..char_h {
            let i_font = y * char_w * nb_chars + x + c_index * char_w;
            if font.fontmap[i_font] > 0 {
                fb.get_pixel_mut(x0 + x as u32, y0 + y as u32).copy_from_slice(&[r, g, b, 0xff]);
            }
        }
    }

}


pub fn draw_rect(fb: &mut Framebuffer, rect: &Rect, color: &Color, alpha: u8) {

    let Rect { x0, y0, w, h } = *rect;
    let Color(r, g, b) = *color;

    for x in x0..x0+w {
        for y in y0..y0+h {
            let pixel = fb.get_pixel_mut(x, y);
            pixel[0] = blend(pixel[0], r, alpha);
            pixel[1] = blend(pixel[1], g, alpha);
            pixel[2] = blend(pixel[2], b, alpha);
        }
    }
}

fn blend(a: u8, b: u8, alpha: u8) -> u8 {

    let a = a as u16;
    let b = b as u16;
    let alpha = alpha as u16;

    let r = a * (256 - alpha) + b * (1 + alpha);

    (r >> 8) as u8
}
