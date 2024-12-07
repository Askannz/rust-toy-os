use alloc::format;
use applib::{FbView, OwnedPixels, Framebuffer};
use applib::drawing::primitives::draw_rect;
use applib::drawing::text::{draw_str, draw_line_in_rect, TextJustification};
use applib::uitk::{BarValue, HorizBarConfig, UiContext};
use applib::{Color, Rect};
use applib::{uitk::{self}, FbViewMut};
use chrono::{DateTime, Datelike, NaiveDate, TimeZone, Timelike, Utc, Month};
use num_traits::float::FloatCore;

use crate::resources;
use crate::stats::SystemStats;

pub fn topbar<'a, F: FbViewMut>(
    uitk_context: &mut uitk::UiContext<F>,
    system_stats: &SystemStats,
    datetime: DateTime<Utc>,
) {

    const TOPBAR_H: u32 = 40;

    let font = uitk_context.font_family.get_default();

    let UiContext { fb, stylesheet, .. } = uitk_context;

    let (w, _h) = fb.shape();

    let topbar_rect = Rect { x0: 0, y0: 0, w, h: TOPBAR_H };

    draw_rect(
        *fb,
        &topbar_rect,
        stylesheet.colors.background,
        false
    );


    //
    // Date and time

    let month_str = Month::try_from(datetime.month0() as u8).unwrap().name();

    let day_suffix = match datetime.day() % 10 {
        1 => "st",
        2 => "nd",
        _ => "th"
    };

    let clock_str = format!(
        "{}, {} {}{}, {:02}:{:02}",
        datetime.weekday(),
        month_str,
        datetime.day(),
        day_suffix,
        datetime.hour(),
        datetime.minute()
    );

    draw_line_in_rect(
        *fb,
        &clock_str,
        &topbar_rect,
        font,
        stylesheet.colors.text,
        TextJustification::Right
    );


    //
    // Resources

    const FRAMETIME_WINDOW_LEN: usize = 50;
    const RESOURCES_BAR_W: u32 = 100;
    const RESOURCES_BAR_H: u32 = 10;
    const SEP_MARGIN_W: u32 = 30;
    const ICON_MARGIN_W: u32 = 5;

    struct ResourceMonitor<'a> {
        bar_values: &'a [BarValue],
        max_val: f32,
        icon: &'a Framebuffer<OwnedPixels>,
    }

    let mut x = 0;

    let mut draw_monitor = |monitor: &ResourceMonitor| {

        x += ICON_MARGIN_W as i64;
        let (icon_w, icon_h) = monitor.icon.shape();
        let icon_rect = Rect { 
            x0: x, y0: 0,
            w: icon_w, h: icon_h
        }.align_to_rect_vert(&topbar_rect);
        uitk_context.fb.copy_from_fb(
            monitor.icon,
            (icon_rect.x0, icon_rect.y0),
            true
        );
    
        x += icon_rect.w as i64;
        x += ICON_MARGIN_W as i64;
    
        let res_bar_rect = Rect { 
            x0: x, y0: 0,
            w: RESOURCES_BAR_W, h: RESOURCES_BAR_H
        }.align_to_rect_vert(&topbar_rect);
    
        uitk_context.horiz_bar(
            &HorizBarConfig { max_val: monitor.max_val, rect: res_bar_rect },
            monitor.bar_values,
        );

        x += RESOURCES_BAR_W as i64;
        x += SEP_MARGIN_W as i64;

    };

    let frametime_data = system_stats.get_system_history(|dp| dp.frametime_used as f32);
    let agg_frametime = frametime_data.iter()
        .take(FRAMETIME_WINDOW_LEN)
        .fold(0.0, |acc, v| acc + v / frametime_data.len() as f32);

    let bar_color = {
        if agg_frametime < 5.0 {
            Color::GREEN
        } else if agg_frametime < 10.0 {
            Color::YELLOW
        } else {
            Color::RED
        }
    };

    draw_monitor(&ResourceMonitor { 
        bar_values: &[BarValue { color: bar_color, val: agg_frametime }],
        max_val: 1000.0 / 60.0,
        icon: &resources::SPEEDOMETER_ICON,
    });


    let mem_data = system_stats.get_system_history(|dp| dp.heap_usage as f32);
    let agg_mem = mem_data.iter()
        .fold(0.0, |acc, v| acc + v / mem_data.len() as f32);

    draw_monitor(&ResourceMonitor { 
        bar_values: &[BarValue { color: Color::AQUA, val: agg_mem }],
        max_val: system_stats.heap_total as f32,
        icon: &resources::CHIP_ICON,
    });


    let net_sent_data = system_stats.get_system_history(|dp| dp.net_sent as f32);
    let net_recv_data = system_stats.get_system_history(|dp| dp.net_recv as f32);
    let agg_net_sent = net_sent_data.iter().sum::<f32>();
    let agg_net_recv = net_recv_data.iter().sum::<f32>();

    draw_monitor(&ResourceMonitor { 
        bar_values: &[
            BarValue { color: Color::YELLOW, val: agg_net_sent },
            BarValue { color: Color::BLUE, val: agg_net_recv },
        ],
        max_val: 1000.0,
        icon: &resources::NETWORK_ICON,
    });
}
