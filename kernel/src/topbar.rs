use alloc::format;
use applib::drawing::primitives::draw_rect;
use applib::drawing::text::{draw_str};
use applib::uitk::UiContext;
use applib::Rect;
use applib::{uitk::{self}, FbViewMut};
use chrono::{DateTime, Datelike, NaiveDate, TimeZone, Timelike, Utc, Month};

pub fn topbar<'a, F: FbViewMut>(
    uitk_context: &mut uitk::UiContext<F>,
    datetime: DateTime<Utc>,
) {

    const BAR_H: u32 = 40;

    let font = uitk_context.font_family.get_default();

    let UiContext { fb, stylesheet, .. } = uitk_context;

    let (w, _h) = fb.shape();

    let bar_rect = Rect { x0: 0, y0: 0, w, h: BAR_H };

    draw_rect(
        *fb,
        &bar_rect,
        stylesheet.colors.background,
        false
    );

    let char_h = font.char_h as u32;

    let margin = (BAR_H - char_h) / 2;

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

    let text_w = (clock_str.len() * font.char_w) as u32;
    let text_x0 = (w - text_w - margin) as i64;
    let text_y0 = margin as i64;

    draw_str(
        *fb,
        &clock_str,
        text_x0,text_y0,
        font,
        stylesheet.colors.text,
        None
    );
}
