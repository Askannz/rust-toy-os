#![no_std]

#[derive(Debug, Clone)]
#[repr(C)]
pub struct SystemState {
    pub pointer: PointerState,
    pub time: u64,
}

#[derive(Debug, Clone)]
#[repr(C)]
pub struct PointerState {
    pub x: i32,
    pub y: i32,
    pub clicked: bool
}

#[derive(Clone)]
pub struct Color(pub u8, pub u8, pub u8);
#[derive(Clone)]
pub struct Rect { pub x0: i32, pub y0: i32, pub w: i32, pub h: i32 }

impl Rect {
    pub fn check_in(&self, x: i32, y: i32) -> bool {
        return 
            x >= self.x0 && x < self.x0 + self.w &&
            y >= self.y0 && y < self.y0 + self.h
    }
}

pub struct Framebuffer<'a> {
    pub data: &'a mut [u8],
    pub w: i32,
    pub h: i32,
}

pub struct FrameBufRegion<'a, 'b> {
    pub fb: &'b mut Framebuffer<'a>,
    pub rect: Rect
}

impl<'a> Framebuffer<'a> {

    pub fn get_region<'b>(&'b mut self, rect: &Rect) -> FrameBufRegion<'a, 'b> {
        // TODO: bounds check
        FrameBufRegion { fb: self, rect: rect.clone() }
    }

    pub fn as_region<'b>(&'b mut self) -> FrameBufRegion<'a, 'b> {
        let rect = Rect { x0: 0, y0: 0, w: self.w, h: self.h };
        FrameBufRegion { fb: self, rect }
    }

    fn get_pixel_mut(&mut self, x: i32, y: i32) -> &mut [u8] {
        // TODO: bounds check
        let i = (y * self.w + x) as usize * 4;
        &mut self.data[i..i+4]
    }

    fn get_pixel(&self, x: i32, y: i32) -> &[u8] {
        // TODO: bounds check
        let i = (y * self.w + x) as usize * 4;
        &self.data[i..i+4]
    }
}

impl<'a, 'b> FrameBufRegion<'a, 'b> {


    pub fn get_pixel_mut(&mut self, x: i32, y: i32) -> &mut [u8] {
        // TODO: bounds check
        let x_fb = x + self.rect.x0;
        let y_fb = y + self.rect.y0;
        self.fb.get_pixel_mut(x_fb, y_fb)
    }

    pub fn get_pixel(&self, x: i32, y: i32) -> &[u8] {
        // TODO: bounds check
        let x_fb = x + self.rect.x0;
        let y_fb = y + self.rect.y0;
        self.fb.get_pixel(x_fb, y_fb)
    }

    pub fn set_pixel(&mut self, x: i32, y: i32, color: &Color) {
        let &Color(r, g, b) = color;
        self.get_pixel_mut(x, y).copy_from_slice(&[r, g, b, 0xff]);
    }

    pub fn copy_from(&mut self, src: &FrameBufRegion) {

        let w = i32::min(self.rect.w, src.rect.w);
        let h = i32::min(self.rect.h, src.rect.h);

        for x in 0..w {
            for y in 0..h {
                let px_src = src.get_pixel(x, y);
                self.get_pixel_mut(x, y).copy_from_slice(px_src);
            }
        }
    }

    pub fn fill(&mut self, color: &Color) {
    
        let Rect { x0, y0, w, h } = self.rect;
    
        for x in x0..x0+w {
            for y in y0..y0+h {
                self.set_pixel(x, y, color);
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

pub fn draw_str(fb: &mut Framebuffer, s: &str, x0: i32, y0: i32, font: &Font, color: &Color) {
    let mut x = x0;
    for c in s.as_bytes() {
        draw_char(fb, *c, x, y0, font, color);
        x += font.char_w as i32;
    }
}

fn draw_char(fb: &mut Framebuffer, c: u8, x0: i32, y0: i32, font: &Font, color: &Color) {

    // Supported chars
    assert!(c >= 32 && c <= 126);

    let mut fb = fb.as_region();

    let c_index = (c - 32) as usize;
    let Font { nb_chars, char_h, char_w, .. } = *font;

    for x in 0..char_w {
        for y in 0..char_h {
            let i_font = y * char_w * nb_chars + x + c_index * char_w;
            if font.fontmap[i_font] > 0 {
                fb.set_pixel(x0 + x as i32, y0 + y as i32, color);
            }
        }
    }

}


pub fn draw_rect(fb: &mut Framebuffer, rect: &Rect, color: &Color, alpha: u8) {

    let x0 = i32::max(0, rect.x0);
    let x1 = i32::min(fb.w-1, rect.x0+rect.w);
    let y0 = i32::max(0, rect.y0);
    let y1 = i32::min(fb.h-1, rect.y0+rect.h);

    let Color(r, g, b) = *color;
    for x in x0..=x1 {
        for y in y0..=y1 {
            let i = ((y * fb.w + x) * 4) as usize;
            fb.data[i] = blend(fb.data[i], r, alpha);
            fb.data[i+1] = blend(fb.data[i], g, alpha);
            fb.data[i+2] = blend(fb.data[i], b, alpha);
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
