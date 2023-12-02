use alloc::vec::Vec;
use lazy_static::lazy_static;
use zune_png::PngDecoder;
use crate::{Framebuffer, Color, Rect, blend_colors};

#[derive(Debug, Clone)]
pub struct ScreenPoint { pub x: i64, pub y: i64 }

pub fn draw_triangle(fb: &mut Framebuffer, tri: &[ScreenPoint; 3], color: Color) {

    let i = {
        if tri[0].y <= i64::min(tri[1].y, tri[2].y) { 0 }
        else if tri[1].y <= i64::min(tri[0].y, tri[2].y) { 1 }
        else { 2 }
    };

    let p0 = &tri[i];
    let p2 = &tri[(i + 1) % 3];
    let p1 = &tri[(i + 2) % 3];

    let y_half = i64::min(p1.y, p2.y);
    fill_half_triangle(fb, (p0, p1), (p0, p2), (p0.y, y_half), color);

    if p1.y < p2.y {
        fill_half_triangle(fb, (p1, p2), (p0, p2), (y_half, p2.y), color);
    } else {
        fill_half_triangle(fb, (p0, p1), (p2, p1), (y_half, p1.y), color);
    }
}

#[inline]
fn fill_half_triangle(
    fb: &mut Framebuffer,
    left: (&ScreenPoint, &ScreenPoint), right: (&ScreenPoint, &ScreenPoint),
    range: (i64, i64),
    color: Color
) {

    let (pl0, pl1) = left;
    let (pr0, pr1) = right;
    let (y_min, y_max) = range;

    if pl0.y == pl1.y || pr0.y == pr1.y { return; }

    let f_left = (pl1.x - pl0.x) as f32 / (pl1.y - pl0.y) as f32;
    let f_right = (pr1.x - pr0.x) as f32 / (pr1.y - pr0.y) as f32;

    for y in y_min..=y_max {
        let x_min = ((y - pl0.y) as f32 * f_left) as i64 + pl0.x;
        let x_max = ((y - pr0.y) as f32 * f_right) as i64 + pr0.x;
        if x_min <= x_max {
            let line_w = x_max - x_min + 1;
            fb.fill_line(x_min as u32, line_w as u32, y as u32, color);
        }
    }
}

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
        bitmap_png: include_bytes!("../fontmap.png"),
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


pub fn draw_rect(fb: &mut Framebuffer, rect: &Rect, color: Color) {
    let Rect { x0, y0, w, h } = *rect;
    for y in y0..y0+h {
        fb.fill_line(x0, w, y, color);
    }
}

pub fn blend_rect(fb: &mut Framebuffer, rect: &Rect, color: Color) {

    let Rect { x0, y0, w, h } = *rect;

    for y in y0..y0+h {
        for x in x0..x0+w {
            let current = fb.get_pixel(x, y);
            let new = blend_colors(color, current);
            fb.set_pixel(x, y, new);
        }
    }
}

