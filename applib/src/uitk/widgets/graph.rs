use crate::drawing::primitives::draw_rect;
use crate::drawing::text::{draw_str, Font};
use crate::uitk::{UiContext};
use crate::{Color, FbView, FbViewMut, Framebuffer, OwnedPixels, Rect};
use alloc::borrow::ToOwned;
use alloc::string::String;
use num::traits::float::FloatCore;

impl<'a, F: FbViewMut> UiContext<'a, F> {

    pub fn graph(&mut self, config: &GraphConfig, series_list: &[GraphSeries]) {

        if let Some(bg_color) = config.bg_color {
            draw_rect(self.fb, &config.rect, bg_color, false);
        }

        let mut draw_series = |series: &GraphSeries| {

            let Rect { x0, y0, w, h } = config.rect;

            let n = series.data.len();

            if n < w as usize { unimplemented!() }
            let w_f = w as f32;
            let h_f = h as f32;

            let win_size: f32 = n as f32 / w_f;

            for x in 0..(w as i64) {
                let x_f = x as f32;
                let i1_f = x_f * win_size;
                let i2_f = i1_f + win_size;

                let i1 = f32::round(i1_f);
                let i2 = f32::round(i2_f);

                let real_win_size = i2 - i1;

                let i1 = i1 as usize;
                let i2 = i2 as usize;

                let window = &series.data[i1..i2];

                let agg_val = match series.agg_mode {
                    GraphAggMode::MIN => window.iter().fold(0.0, |acc, v| f32::min(acc, *v)),
                    GraphAggMode::MAX => window.iter().fold(0.0, |acc, v| f32::max(acc, *v)),
                    GraphAggMode::AVG => window.iter().fold(0.0, |acc, v| acc + v / real_win_size),
                    GraphAggMode::SUM => window.iter().sum(),
                };

                let dy = f32::round(h_f * agg_val / config.max_val) as u32;

                let graph_rect = Rect { 
                    x0: x + x0,
                    y0: y0 + (h - dy) as i64,
                    w: 1,
                    h: dy,
                };

                draw_rect(self.fb, &graph_rect, series.color, false);
            }
        };

        for series in series_list {
            draw_series(series);
        }

    }
}

pub struct GraphSeries<'a> {
    pub data: &'a [f32],
    pub color: Color,
    pub agg_mode: GraphAggMode,
}

#[derive(Debug, Clone, Copy)]
pub enum GraphAggMode { MIN, MAX, AVG, SUM }

#[derive(Clone)]
pub struct GraphConfig {
    pub rect: Rect,
    pub max_val: f32,
    pub bg_color: Option<Color>,
}

impl Default for GraphConfig {
    fn default() -> Self {
        GraphConfig {
            rect: Rect {
                x0: 0,
                y0: 0,
                w: 100,
                h: 25,
            },
            max_val: 100.0,
            bg_color: None,
        }
    }
}
