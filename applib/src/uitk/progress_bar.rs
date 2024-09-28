use crate::drawing::primitives::draw_rect;
use crate::drawing::text::{draw_str, Font, DEFAULT_FONT};
use crate::uitk::UiContext;
use crate::{Color, FbViewMut, Rect};

impl<'a, F: FbViewMut> UiContext<'a, F> {
    pub fn progress_bar(&mut self, config: &ProgressBarConfig, progress: u64, text: &str) {
        let UiContext { fb, .. } = self;

        let Rect { x0, y0, h, w } = config.rect;
        draw_rect(*fb, &config.rect, config.bg_color, false);

        let p = config.bar_padding;
        let bar_w = (((w - 2 * p) as u64) * progress / config.max_val) as u32;
        let bar_rect = Rect {
            x0: x0 + p as i64,
            y0: y0 + p as i64,
            h: h - 2 * p,
            w: bar_w,
        };

        draw_rect(*fb, &bar_rect, config.bar_color, false);

        let text_x_padding: i64 = config.text_x_padding.into();
        let &Font { char_h, .. } = config.font;
        let char_h = char_h as i64;
        let h: i64 = h.into();

        let text_x0 = x0 + text_x_padding;
        let text_y0 = y0 + i64::max(0, (h - char_h) / 2);

        draw_str(
            *fb,
            &text,
            text_x0,
            text_y0,
            config.font,
            config.text_color,
            None,
        );
    }
}

#[derive(Clone)]
pub struct ProgressBarConfig {
    pub rect: Rect,
    pub font: &'static Font,
    pub bg_color: Color,
    pub text_color: Color,
    pub bar_color: Color,
    pub bar_padding: u32,
    pub text_x_padding: u32,
    pub max_val: u64,
}

impl Default for ProgressBarConfig {
    fn default() -> Self {
        ProgressBarConfig {
            rect: Rect {
                x0: 0,
                y0: 0,
                w: 100,
                h: 25,
            },
            font: &DEFAULT_FONT,
            bg_color: Color::BLACK,
            text_color: Color::WHITE,
            bar_color: Color::BLUE,
            bar_padding: 2,
            text_x_padding: 10,
            max_val: 100,
        }
    }
}
