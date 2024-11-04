use crate::drawing::primitives::draw_rect;
use crate::drawing::text::{draw_str, Font};
use crate::uitk::UiContext;
use crate::{Color, FbViewMut, Rect};

impl<'a, F: FbViewMut> UiContext<'a, F> {
    pub fn progress_bar(&mut self, config: &ProgressBarConfig, progress: u64, text: &str) {
        let UiContext { fb, stylesheet, font_family, .. } = self;

        let colorsheet = &stylesheet.colors;

        let container_rect = {
            let m = stylesheet.margin as i64;
            let [x0, y0, x1, y1] = config.rect.as_xyxy();
            Rect::from_xyxy([x0+m, y0+m, x1-m, y1-m])
        };

        let bar_w = ((container_rect.w as u64) * progress / config.max_val) as u32;

        let bar_rect = {
            let Rect { x0, y0, h, .. } = container_rect;
            Rect { x0, y0, h, w: bar_w }
        };

        draw_rect(*fb, &config.rect, colorsheet.background, false);
        draw_rect(*fb, &bar_rect, colorsheet.element, false);

        let font = font_family.get_default();

        let text_w = (text.len() * font.char_w) as u32;
        let text_h = font.char_h as u32;

        let text_rect = Rect { 
            x0: container_rect.x0 + stylesheet.margin as i64,
            y0: 0,
            w: text_w,
            h: text_h
        }.align_to_rect_vert(&container_rect);

        draw_str(
            *fb,
            &text,
            text_rect.x0,
            text_rect.y0,
            font,
            colorsheet.text,
            None,
        );
    }
}

#[derive(Clone)]
pub struct ProgressBarConfig {
    pub rect: Rect,
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
            max_val: 100,
        }
    }
}
