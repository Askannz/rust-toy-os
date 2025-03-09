use crate::drawing::primitives::draw_rect;
use crate::drawing::text::{draw_line_in_rect, draw_str, Font, FontFamily, TextJustification};
use crate::uitk::{ContentId, UiContext};
use crate::{Color, FbView, FbViewMut, Framebuffer, OwnedPixels, Rect, StyleSheet};
use alloc::borrow::ToOwned;
use alloc::string::String;
use num::traits::real::Real;

impl<'a, F: FbViewMut> UiContext<'a, F> {

    pub fn section<S>(&mut self, rect: &Rect, text: &str, mut func: S) 
        where S: FnMut(&mut Self, &Rect)
    {

        const MARGIN: u32 = 10;

        let Rect { x0, y0, w, h } = *rect;
        let font = self.font_family.get_default();
        let color = Color::WHITE;

        let text_rect = Rect { 
            x0: x0 + MARGIN as i64, y0: y0 + MARGIN as i64,
            w: w - MARGIN, h: font.char_h as u32
        };
        let (_, text_x1) = draw_line_in_rect(
            self.fb, text, &text_rect, font, color, TextJustification::Left);

        let (_, yc) = text_rect.center();
        let line_rect = Rect::from_xyxy([
            text_x1 + MARGIN as i64, yc,
            x0 + (w - MARGIN) as i64 - 1, yc,
        ]);
        draw_rect(self.fb, &line_rect, color, false);

        let inner_rect = Rect {
            x0: x0 + MARGIN as i64,
            y0: y0 + (text_rect.h + MARGIN) as i64,
            w: w - 2 * MARGIN,
            h: h - (2 * MARGIN + text_rect.h)
        };

        func(self, &inner_rect);
    }

    pub fn layout_horiz<S>(&mut self, rect: &Rect, count: usize, mut func: S) 
        where S: FnMut(&mut Self, usize, &Rect)
    {
        const MARGIN: u32 = 5;

        let Rect { x0, y0, w, h } = *rect;

        let button_total_w = w - (count as u32 - 1) * MARGIN;
        let element_h = h - 2 * MARGIN;

        let mut x = x0;
        for i in 0..count {

            let element_w = {
                let x0 = (i as f32) / (count as f32) * (button_total_w as f32);
                let x1 = ((i + 1) as f32) / (count as f32) * (button_total_w as f32);

                let x0 = f32::round(x0) as u32;
                let x1 = f32::round(x1) as u32;

                x1 - x0
            };

            let element_rect = Rect { 
                x0: x, y0: y0 + MARGIN as i64,
                w: element_w, h: element_h,
            };

            func(self, i, &element_rect);

            x += (element_w + MARGIN) as i64;
        }

    }

    pub fn layout_box<S>(
        &mut self, rect: &Rect,
        left: f32, top: f32, right: f32, bottom: f32,
        mut func: S
    ) 
        where S: FnMut(&mut Self, &Rect)
    {
        
        let Rect { w, h, .. } = *rect;
        let [x0, y0, x1, y1] = rect.as_xyxy();

        let dx0 = f32::round(left * (w as f32)) as i64;
        let dx1 = f32::round(right * (w as f32)) as i64;
        let dy0 = f32::round(top * (h as f32)) as i64;
        let dy1 = f32::round(bottom * (h as f32)) as i64;

        let box_rect = Rect::from_xyxy([x0+dx0, y0+dy0, x1-dx1, y1-dy1]);

        func(self, &box_rect)
    }
}