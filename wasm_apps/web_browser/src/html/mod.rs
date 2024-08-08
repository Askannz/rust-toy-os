use anyhow::anyhow;
use applib::{Color, Rect, Framebuffer};

pub mod parsing;
pub mod layout;
pub mod render;

pub fn render_html(fb: &mut Framebuffer, html: &str) -> anyhow::Result<()> {
    let html_tree = parsing::parse_html(html)?;
    let layout = layout::compute_layout(&html_tree)?;
    render::render_html(fb, &layout);
    Ok(())
}
