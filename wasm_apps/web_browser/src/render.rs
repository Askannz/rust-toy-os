use std::error::Error;

use applib::{Color, Rect, Framebuffer};
use applib::drawing::primitives::{draw_rect, blend_rect};
use applib::drawing::text::draw_str;
use applib::drawing::text::{HACK_15, Font};
use applib::input::{InputState, InputEvent, PointerState};

use crate::html_parsing::{parse_html, HtmlTree, NodeId, NodeData as HtmlNodeData};
use crate::errors::HtmlError;

pub struct Webview<'a> {
    state: State,
    view_rect: Rect,
    next_node_id: u64,
    buffer: Framebuffer<'a>,
}

enum State {
    Blank,
    Active {
        layout: LayoutNode,
        link_data: Option<LinkData>,
        y_offset: i64,
    },
    Error { msg: String }
}

struct LinkData {
    node_id: u64,
    rect: Rect,
    url: String,
    clicked: bool,
}

const SCROLL_SPEED: u32 = 20;
const MAX_RENDER_HEIGHT: u32 = 3_000;

impl<'a> Webview<'a> {

    pub fn new(view_rect: &Rect) -> Self {
        Self {
            state: State::Blank,
            view_rect: view_rect.clone(),
            next_node_id: 0,
            buffer: Framebuffer::new_owned(view_rect.w, MAX_RENDER_HEIGHT),
        }
    }

    pub fn update(&mut self, input_state: &InputState, html_update: Option<&str>) -> bool {

        let mut redraw = false;

        if let Some(html) = html_update {

            self.state = match self.parse_html_to_layout(html) {

                Ok(layout) => {

                    //debug_layout(&layout);

                    self.buffer.fill(Color::WHITE);
                    draw_node(&mut self.buffer, &layout);
        
                    redraw = true;

                    State::Active {
                        layout,
                        link_data: None,
                        y_offset: 0,
                    }
                },

                Err(error) => {
                    redraw = true;
                    State::Error { msg: error.to_string() }
                }
            }
        }

        match &mut self.state {
            State::Active { layout, link_data, y_offset, .. } => {

                for event in input_state.events {
                    if let Some(InputEvent::Scroll { delta }) = event {
                        let offset = *y_offset as i64 - delta * (SCROLL_SPEED as i64);
                        *y_offset = i64::max(0, offset);
                        redraw = true;
                    }
                }

                let PointerState { mut x, mut y, ..} = input_state.pointer;
                y = y - self.view_rect.y0 + *y_offset;
                x = x - self.view_rect.x0;

                let new_link_data = get_hovered_link(x, y, layout);

                match (&new_link_data, &link_data) {
                    (Some(new), Some(old)) if new.node_id == old.node_id => (),
                    (None, None) => (),
                    _ => { redraw = true; }
                };

                *link_data = new_link_data;

                if let Some(link_data_val) = link_data {
                    link_data_val.clicked = input_state.pointer.left_clicked;
                }
            },
            _ => (),
        }

        redraw
    }

    pub fn draw(&self, fb: &mut Framebuffer) {

        match &self.state {
            State::Blank => (),
            State::Active { y_offset, link_data, .. } => {
                let src_rect = {
                    let mut r = self.buffer.shape_as_rect().clone();
                    r.y0 += y_offset;
                    r
                };
    
                fb.copy_from_fb(&self.buffer, &src_rect, &self.view_rect, false);
    
                if let Some(link_data) = link_data {
                    let mut r = link_data.rect.clone();
                    r.y0 = r.y0 + self.view_rect.y0 - y_offset;
                    r.x0 = r.x0 + self.view_rect.x0;
                    blend_rect(fb, &r, Color::rgba(0, 0, 255, 128));
                }
            },
            State::Error { msg } => {
                let Rect { x0, y0, .. } = self.view_rect;
                draw_str(fb, msg, x0, y0, &HACK_15, Color::BLACK, None);
            }
        }
    }

    pub fn check_redirect(&self) -> Option<&str> {
        if let State::Active { link_data, .. } = &self.state {
            if let Some(link_data) = link_data {
                if link_data.clicked {
                    return Some(&link_data.url)
                }
            }
        }
        None
    }

    fn parse_html_to_layout(&mut self, html: &str) -> Result<LayoutNode, HtmlError> {

        let tree = parse_html(html)?;
        self.parse_node(&tree, NodeId(0), 0, 0, false)
            .ok_or(HtmlError::new("Error computing HTML layout"))
    }

    fn parse_node<'b>(&mut self, tree: &HtmlTree, node_id: NodeId, mut x0: i64, mut y0: i64, link: bool) -> Option<LayoutNode> {

        const ZERO_M: Margins = Margins { left: 0, right: 0, top: 0, bottom: 0};
        const TR_M: Margins  = Margins { left: 0, right: 0, top: 5, bottom: 5};

        let node = tree.get_node(node_id).unwrap();
    
        match &node.data {

            HtmlNodeData::Tag { name, .. } if *name == "head" => None,

            HtmlNodeData::Tag { name, attrs, .. } if *name == "img" => {
                let parse_dim = |attr: &str| -> u32 { attrs.get(attr).map(|s| s.parse().ok()).flatten().unwrap_or(0) };
                let w: u32 = parse_dim("width");
                let h: u32 = parse_dim("height");
                Some(LayoutNode {
                    id: self.make_node_id(),
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

                    if let Some(child_node) = self.parse_node(tree, *html_child_id, child_x0, child_y0, url.is_some()) {

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
                        id: self.make_node_id(),
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
                    id: self.make_node_id(),
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

    fn make_node_id(&mut self) -> u64 {
        let id = self.next_node_id;
        self.next_node_id += 1;
        id
    }
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

fn draw_node(fb: &mut Framebuffer, node: &LayoutNode) {

    let rect = &node.rect;

    if fb.w as i64 <= rect.x0 || fb.h as i64 <= rect.y0 {
        return;
    }

    match &node.data {
        NodeData::Text { text, color, font, .. } => {
            draw_str(fb, text, rect.x0, rect.y0, font, *color, None);
        },
        NodeData::Image => (),
        NodeData::Container { children, bg_color, .. } => {

            if let &Some(bg_color) = bg_color {
                draw_rect(fb, &rect, bg_color);
            }

            for child in children.iter() {
                draw_node(fb, child);
            }
        }
    }
}

fn get_hovered_link(x: i64, y: i64, node: &LayoutNode) -> Option<LinkData> {

    let rect = &node.rect;

    match &node.data {
        NodeData::Container { children, url, .. } => match rect.check_contains_point(x, y) {
            true => match url {
                Some(url) => Some(LinkData {
                    node_id: node.id,
                    rect: rect.clone(),
                    url: url.clone(),
                    clicked: false,
                }),
                None => children.iter().find_map(|c| get_hovered_link(x, y, c))
            },
            false => None
        },
        _ => None,
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

enum NodeData {
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

struct LayoutNode {
    id: u64,
    rect: Rect,
    data: NodeData,
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

fn check_is_whitespace(s: &str) -> bool {
    s.chars().map(|c| char::is_whitespace(c)).all(|x| x)
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