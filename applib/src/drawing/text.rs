use alloc::vec::Vec;
use lazy_static::lazy_static;
use zune_png::PngDecoder;
use crate::{Framebuffer, Color, Rect};

struct FontSpec {
    bitmap_png: &'static [u8],
    nb_chars: usize,
    char_h: usize,
    char_w: usize,
}

impl FontSpec {
    fn load(&self) -> Font {

        let FontSpec { nb_chars, char_h, char_w, .. } = *self;

        let bitmap = PngDecoder::new(self.bitmap_png)
            .decode().expect("Invalid PNG bitmap")
            .u8().expect("Invalid PNG bitmap");

        if bitmap.len() != nb_chars * char_w * char_h {
            panic!("Invalid font bitmap size");
        }

        Font { bitmap, nb_chars, char_h, char_w }
    }
}

pub struct Font {
    bitmap: Vec<u8>,
    pub nb_chars: usize,
    pub char_h: usize,
    pub char_w: usize,
}


lazy_static! {
    pub static ref DEFAULT_FONT: Font = FontSpec {
        bitmap_png: include_bytes!("../../fonts/default.png"),
        nb_chars: 95,
        char_h: 24,
        char_w: 12,
    }.load();
}

pub fn draw_text_rect(fb: &mut Framebuffer, s: &str, rect: &Rect, font: &Font, color: Color) {
    
    let Rect { x0, y0, w, .. } = *rect;
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

pub fn draw_str(fb: &mut Framebuffer, s: &str, x0: u32, y0: u32, font: &Font, color: Color) {
    let mut x = x0;
    for c in s.as_bytes() {
        draw_char(fb, *c, x, y0, font, color);
        x += font.char_w as u32;
    }
}

fn draw_char(fb: &mut Framebuffer, mut c: u8, x0: u32, y0: u32, font: &Font, color: Color) {

    // Replacing unsupported chars with spaces
    if c < 32 || c > 126 { c = 32}

    let c_index = (c - 32) as usize;
    let Font { nb_chars, char_h, char_w, .. } = *font;

    for x in 0..char_w {
        for y in 0..char_h {
            let i_font = y * char_w * nb_chars + x + c_index * char_w;
            if font.bitmap[i_font] > 0 {
                fb.set_pixel(x0 + x as u32, y0 + y as u32, color);
            }
        }
    }

}