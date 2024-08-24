use alloc::vec::Vec;
use alloc::string::String;
use lazy_static::lazy_static;
use crate::{Framebuffer, Color, Rect, blend_colors, decode_png};

struct FontSpec {
    bitmap_png: &'static [u8],
    nb_chars: usize,
    char_h: usize,
    char_w: usize,
    base_y: usize,
}

impl FontSpec {
    fn load(&self) -> Font {

        let FontSpec { nb_chars, char_h, char_w, base_y, .. } = *self;

        let bitmap = decode_png(self.bitmap_png);

        if bitmap.len() != nb_chars * char_w * char_h {
            panic!("Invalid font bitmap size");
        }

        Font { bitmap, nb_chars, char_h, char_w, base_y }
    }
}

#[derive(Hash)]
pub struct Font {
    bitmap: Vec<u8>,
    pub nb_chars: usize,
    pub char_h: usize,
    pub char_w: usize,
    pub base_y: usize,
}


lazy_static! {
    pub static ref DEFAULT_FONT: Font = FontSpec {
        bitmap_png: include_bytes!("../../fonts/default.png"),
        nb_chars: 95,
        char_h: 24,
        char_w: 12,
        base_y: 19
    }.load();

    pub static ref HACK_15: Font = FontSpec {
        bitmap_png: include_bytes!("../../fonts/hack_15.png"),
        nb_chars: 95,
        char_h: 18,
        char_w: 10,
        base_y: 14,
    }.load();
}

pub fn draw_str(fb: &mut Framebuffer, s: &str, x0: i64, y0: i64, font: &Font, color: Color, bg_color: Option<Color>) {
    let mut x = x0;
    for c in s.chars() {
        draw_char(fb, c, x, y0, font, color, bg_color, true);
        x += font.char_w as i64;
    }
}

pub fn draw_char(fb: &mut Framebuffer, c: char, x0: i64, y0: i64, font: &Font, color: Color, bg_color: Option<Color>, blend: bool) {

    let mut c = c as u8;

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
            let is_in_font = val_font > 0;

            let txt_color = Color::rgba(r, g, b, val_font);

            let new_color = match blend {

                true => match fb.get_pixel(x, y) {
                    Some(curr_color) => match is_in_font {
                        true => Some(blend_colors(txt_color, curr_color)),
                        false => bg_color.map(|bg_color| blend_colors(bg_color, curr_color))
                    }
                    None => None
                }

                false => match is_in_font {
                    true => Some(txt_color),
                    false => bg_color
                }
            };

            if let Some(new_color) = new_color {
                fb.set_pixel(x, y, new_color);
            }
        }
    }
}

#[derive(Clone)]
pub struct RichText(Vec<RichChar>);

impl RichText {
    pub fn new() -> Self {
        RichText(Vec::new())
    }

    pub fn add_part(&mut self, s: &str, color: Color, font: &'static Font, bg_color: Option<Color>) {
        self.0.extend(s.chars().map(|c| RichChar { c, color, font, bg_color }));
    }

    pub fn from_str(s: &str, color: Color, font: &'static Font, bg_color: Option<Color>) -> Self {
        let mut t = Self::new();
        t.add_part(s, color, font, bg_color);
        t
    }

    pub fn as_string(&self) -> String {
        let mut s = String::new();
        for rich_char in self.0.iter() {
            s.push(rich_char.c);
        }
        s
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }
}

#[derive(Clone)]
pub struct RichChar {
    c: char,
    color: Color,
    font: &'static Font,
    bg_color: Option<Color>,
}

pub struct FormattedRichLine {
    pub chars: Vec<RichChar>,
    pub w: u32,
    pub h: u32,
}

pub struct FormattedRichText {
    pub lines: Vec<FormattedRichLine>,
    pub w: u32,
    pub h: u32,
}

pub fn format_rich_lines(text: &RichText, max_w: u32) -> FormattedRichText {

    let rich_vec = &text.0;

    if rich_vec.is_empty() { return FormattedRichText { lines: Vec::new(), w: 0, h: 0 }; }
    let max_char_w = rich_vec.iter().map(|rich_char| rich_char.font.char_w).max().unwrap();

    let max_per_line = max_w as usize / max_char_w;

    let mut formatted_lines = Vec::new();
    let (mut text_w, mut text_h) = (0, 0);
    let mut i0 = 0;

    for (i, rich_char) in rich_vec.iter().enumerate() {

        let i1 = {
            if rich_char.c == '\n' { Some(i) }
            else if i - i0 + 1 >= max_per_line || i == rich_vec.len() - 1 { Some(i+1) }
            else { None }
        };

        if let Some(i1) = i1 {

            let rich_slice = &rich_vec[i0..i1];

            let line_h = rich_slice.iter()
                .map(|rich_char| rich_char.font.char_h)
                .max()
                .unwrap_or(0) as u32;

            let line_w = rich_slice.iter()
                .map(|rich_char| rich_char.font.char_w)
                .sum::<usize>() as u32;

            formatted_lines.push(FormattedRichLine {
                chars: rich_slice.to_vec(),
                w: line_w,
                h: line_h,
            });

            text_w = u32::max(text_w, line_w);
            text_h += line_h;

            i0 = i + 1;
        }
    }

    FormattedRichText { lines: formatted_lines, w: text_w, h: text_h }
}

pub fn draw_rich_slice(fb: &mut Framebuffer, rich_slice: &[RichChar], x0: i64, y0: i64) {

    if rich_slice.is_empty() { return; }

    let max_base_y = rich_slice.iter().map(|rich_char| rich_char.font.base_y).max().unwrap();

    let mut x = x0;
    for rich_char in rich_slice.iter() {
        let dy = (max_base_y - rich_char.font.base_y)  as i64;
        draw_char(fb, rich_char.c, x, y0 + dy, rich_char.font, rich_char.color, rich_char.bg_color, true);
        x += rich_char.font.char_w as i64;
    }
}
