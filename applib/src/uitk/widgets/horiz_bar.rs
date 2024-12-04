use crate::drawing::primitives::draw_rect;
use crate::drawing::text::{draw_str, Font};
use crate::uitk::{UiContext};
use crate::{Color, FbView, FbViewMut, Framebuffer, OwnedPixels, Rect};
use alloc::borrow::ToOwned;
use alloc::string::String;
use num::traits::float::FloatCore;

impl<'a, F: FbViewMut> UiContext<'a, F> {

    pub fn horiz_bar(&mut self, config: &HorizBarConfig, values: &[BarValue]) {

        const MARGIN: u32 = 2;

        draw_rect(self.fb, &config.rect, self.stylesheet.colors.element, false);

        let container_rect = Rect {
            x0: config.rect.x0,
            y0: config.rect.y0 + MARGIN as i64,
            w: config.rect.w - MARGIN,
            h: config.rect.h - 2 * MARGIN,
        };

        let Rect { x0, y0, w, h } = container_rect;

        let bar_h = h / values.len() as u32;

        for (i, bar_val) in values.iter().enumerate() {

            let y = y0 + bar_h as i64 * i as i64;
            let v = f32::max(0.0, f32::min(config.max_val, bar_val.val));
            let bar_w = f32::round((w as f32) * v / config.max_val) as u32;

            let rect = Rect {  x0, y0: y, w: bar_w, h: bar_h };

            draw_rect(self.fb, &rect, bar_val.color, false);
        }
    }
}

pub struct BarValue {
    pub val: f32,
    pub color: Color,
}

#[derive(Clone)]
pub struct HorizBarConfig {
    pub rect: Rect,
    pub max_val: f32,
}

impl Default for HorizBarConfig {
    fn default() -> Self {
        HorizBarConfig {
            rect: Rect {
                x0: 0,
                y0: 0,
                w: 100,
                h: 25,
            },
            max_val: 100.0,
        }
    }
}
