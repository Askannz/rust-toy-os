use anyhow::anyhow;

use applib::{Color, Rect, Framebuffer};
use applib::drawing::primitives::{draw_rect, blend_rect};
use applib::drawing::text::draw_str;
use applib::drawing::text::{HACK_15, Font};
use applib::input::{InputState, InputEvent, PointerState};


use super::parsing::{parse_html, HtmlTree, NodeId as HtmlNodeId, NodeData as HtmlNodeData};

pub struct LayoutNode {
    pub id: u64,
    pub rect: Rect,
    pub data: NodeData,
}

pub enum NodeData {
    Text { 
        text: String,
        color: Color,
        font: &'static Font,
        url: Option<String>
    },
    Image,
    Container { 
        children: Vec<LayoutNode>,
        orientation: Orientation,
        bg_color: Option<Color>,
        url: Option<String>,
        tag: String,
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Orientation {
    Horizontal,
    Vertical,
}

struct Margins {
    left: u32,
    right: u32,
    top: u32,
    bottom: u32,
}

pub fn compute_layout(html_tree: &HtmlTree) -> anyhow::Result<LayoutNode> {

    let mut next_node_id = 0u64;
    parse_node(&mut next_node_id, &html_tree, HtmlNodeId(0), 0, 0, false)
        .ok_or(anyhow!("Error computing HTML layout"))
    
}

fn make_node_id(next_node_id: &mut u64) -> u64 {
    let new_id = *next_node_id;
    *next_node_id += 1;
    new_id
}

fn parse_node<'b>(next_node_id: &mut u64, tree: &HtmlTree, html_node_id: HtmlNodeId, mut x0: i64, mut y0: i64, link: bool) -> Option<LayoutNode> {

    const ZERO_M: Margins = Margins { left: 0, right: 0, top: 0, bottom: 0};
    const TR_M: Margins  = Margins { left: 0, right: 0, top: 5, bottom: 5};

    let node = tree.get_node(html_node_id).unwrap();

    match &node.data {

        HtmlNodeData::Tag { name, .. } if *name == "head" => None,

        HtmlNodeData::Tag { name, attrs, .. } if *name == "img" => {
            let parse_dim = |attr: &str| -> u32 { attrs.get(attr).map(|s| s.parse().ok()).flatten().unwrap_or(0) };
            let w: u32 = parse_dim("width");
            let h: u32 = parse_dim("height");
            Some(LayoutNode {
                id: make_node_id(next_node_id),
                rect: Rect { x0, y0, w, h },
                data: NodeData::Image,
            })
        },

        HtmlNodeData::Tag { name, attrs, .. } => {

            let bg_color = match attrs.get("bgcolor") {
                Some(hex_str) => Some(parse_hexcolor(hex_str)),
                _ => None
            };

            let url: Option<String> = match name.as_str() {
                "a" => attrs.get("href").cloned(),
                _ => None,
            };

            let tag = name.to_string();

            let (orientation, margin) = match tag.as_str() {
                "tr" => (Orientation::Horizontal, TR_M),
                "td" => (Orientation::Vertical, ZERO_M),
                "tbody" => (Orientation::Vertical, ZERO_M),
                "table" => (Orientation::Vertical, ZERO_M),
                "div" => (Orientation::Vertical, ZERO_M),
                "span" => (Orientation::Horizontal, ZERO_M),
                "p" => (Orientation::Horizontal, ZERO_M),
                _ => (Orientation::Horizontal, ZERO_M)
            };

            x0 += margin.left as i64;
            y0 += margin.top as i64;

            let mut children: Vec<LayoutNode> = Vec::new();
            let (mut child_x0, mut child_y0): (i64, i64) = (x0, y0);

            for html_child_id in node.children.iter() {

                let html_child = tree.get_node(*html_child_id).unwrap();

                let is_block = check_is_block_element(&html_child.data) && orientation == Orientation::Horizontal;

                // TODO: this should only happen is self.parse_node() is successful
                if is_block {
                    if let Some(prev_child) = children.last() {
                        child_y0 += prev_child.rect.h as i64;
                        child_x0 = x0;
                    }
                }

                if let Some(child_node) = parse_node(next_node_id, tree, *html_child_id, child_x0, child_y0, url.is_some()) {

                    let Rect { w: child_w, h: child_h, .. } = child_node.rect;
                    match orientation {
                        Orientation::Horizontal => child_x0 += child_w as i64,
                        Orientation::Vertical => child_y0 += child_h as i64,
                    }

                    children.push(child_node);
                }
            }

            if children.len() > 0 {
                let rect_0 = children[0].rect.clone();
                let mut container_rect = children.iter()
                    .map(|c| c.rect.clone())
                    .fold(rect_0, |acc, r| r.bounding_box(&acc));
                container_rect.w += margin.right;
                container_rect.h += margin.bottom;
                Some(LayoutNode {
                    id: make_node_id(next_node_id),
                    rect: container_rect,
                    data: NodeData::Container { children, orientation, bg_color, url, tag }
                })
            } else {
                None
            }
        },

        HtmlNodeData::Text { text } if check_is_whitespace(&text) => None,

        HtmlNodeData::Text { text } => {

            let m = ZERO_M;

            let text = core::str::from_utf8(text.as_bytes()).expect("Not UTF-8");
            let font = &HACK_15; // TODO
            let color = if link { Color::BLUE } else { Color::BLACK };
            let w = (text.len() * font.char_w) as u32 + m.left + m.right;
            let h = font.char_h as u32  + m.top + m.bottom;

            Some(LayoutNode {
                id: make_node_id(next_node_id),
                rect: Rect { 
                    x0: x0 + m.left as i64,
                    y0: y0 + m.top as i64,
                    w, h
                },
                data: NodeData::Text { 
                    text: text.to_string(),
                    color,
                    font, 
                    url: None,
                }
            })
        },

        _ => None
    }
}

fn parse_hexcolor(hex_str: &str) -> Color {

    let mut color_bytes = hex::decode(hex_str.replace("#", "")).expect("Invalid color");

    match color_bytes.len() {
        3 => color_bytes.push(255),
        4 => (),
        _ => panic!("Invalid color: {:?}", color_bytes)
    };

    let color_bytes: [u8; 4] = color_bytes.try_into().unwrap();

    Color::from_u32(u32::from_le_bytes(color_bytes))
}

fn check_is_whitespace(s: &str) -> bool {
    s.chars().map(|c| char::is_whitespace(c)).all(|x| x)
}

fn check_is_block_element(node_data: &HtmlNodeData) -> bool {
    match node_data {
        HtmlNodeData::Tag { name, .. } => {
            match name.as_str() {
                "p" => true,
                _ => false
            }
        },
        _ => false
    }
}

fn debug_layout(root_node: &LayoutNode) {

    fn repr_node(out_str: &mut String, node: &LayoutNode, is_last: bool, prefix: &str) {

        let c = match is_last {
            true => "└",
            false => "├",
        };

        match &node.data {
            NodeData::Text { text, .. } => {
                for line in text.split("\n") {
                    out_str.push_str(&format!("{}{}{}\n", prefix, c, line));
                }
            },
            NodeData::Image => {
                out_str.push_str(&format!("{}{}IMAGE {:?}\n", prefix, c, node.rect));
            },
            NodeData::Container { children, orientation, tag, .. } => {

                out_str.push_str(&format!("{}{}CONTAINER {} {:?} {:?}\n", prefix, c, tag, orientation, node.rect));

                let c2 = match is_last {
                    true => " ",
                    false => "|",
                };

                let child_prefix = format!("{}{}", prefix, c2);

                for (i, child) in children.iter().enumerate() {
                    let child_is_last = i == children.len() - 1;
                    repr_node(out_str, child, child_is_last, &child_prefix);
                }
            }
        }
    }

    let mut out_str = String::new();
    repr_node(&mut out_str, root_node, false, "");

    guestlib::print_console(&out_str);

}

