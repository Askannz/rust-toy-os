use alloc::vec::Vec;
use lazy_static::lazy_static;
use crate::{Framebuffer, Color, Rect, blend_colors, decode_png};

struct FontSpec {
    bitmap_png: &'static [u8],
    nb_chars: usize,
    char_h: usize,
    char_w: usize,
}

impl FontSpec {
    fn load(&self) -> Font {

        let FontSpec { nb_chars, char_h, char_w, .. } = *self;

        let bitmap = decode_png(self.bitmap_png);

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

    pub static ref HACK_15: Font = FontSpec {
        bitmap_png: include_bytes!("../../fonts/hack_15.png"),
        nb_chars: 95,
        char_h: 18,
        char_w: 10,
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
            y += char_h as i64;
        }
    }
}

pub fn draw_str(fb: &mut Framebuffer, s: &str, x0: i64, y0: i64, font: &Font, color: Color) {
    let mut x = x0;
    for c in s.as_bytes() {
        draw_char(fb, *c, x, y0, font, color);
        x += font.char_w as i64;
    }
}

fn draw_char(fb: &mut Framebuffer, mut c: u8, x0: i64, y0: i64, font: &Font, color: Color) {

    // Replacing unsupported chars with spaces
    if c < 32 || c > 126 { c = 32 }

    let c_index = (c - 32) as usize;
    let Font { nb_chars, char_h, char_w, .. } = *font;
    let (r, g, b, _a ) = color.as_rgba();

    let char_rect = Rect { x0, y0, w: char_w as u32, h: char_h as u32 };

    let [xc0, yc0, xc1, yc1] = char_rect.as_xyxy();

    for x in xc0..=xc1 {
        for y in yc0..=yc1 {
            let i_font = (y - yc0) as usize * char_w * nb_chars + (x - xc0) as usize + c_index * char_w;
            let val_font = font.bitmap[i_font];

            if val_font > 0 {
                if let Some(curr_color) = fb.get_pixel(x, y) {
                    let new_color = blend_colors(Color::from_rgba(r, g, b, val_font), curr_color);
                    fb.set_pixel(x, y, new_color);
                }
            }
        }
    }
}