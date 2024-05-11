use applib::{Color, Rect, Framebuffer};
use applib::drawing::primitives::{draw_rect, blend_rect};
use applib::drawing::text::draw_str;
use applib::drawing::text::{HACK_15, Font};
use applib::input::{InputState, InputEvent, PointerState};


pub struct Webview<'a> {
    buffer: Option<Framebuffer<'a>>,
    layout: Option<LayoutNode>,
    hovered_rect: Option<Rect>,
    view_rect: Rect,
    y_offset: i64,
}

const SCROLL_SPEED: u32 = 10;

impl<'a> Webview<'a> {

    pub fn new(view_rect: &Rect) -> Self {
        Self { 
            buffer: None,
            layout: None,
            hovered_rect: None,
            view_rect: view_rect.clone(),
            y_offset: 0
        }
    }

    pub fn update(&mut self, input_state: &InputState, html_update: Option<&str>) -> bool {

        let mut redraw = false;

        if let Some(html) = html_update {

            let layout = parse_html_to_layout(html);

            //debug_layout(&layout);

            let &Rect { w: bw, h: bh, .. } = layout.get_rect();
            let mut buffer = Framebuffer::new_owned(bw, bh);
    
            draw_node(&mut buffer, &layout);
    
            self.buffer = Some(buffer);
            self.layout = Some(layout);

            redraw = true;
        }

        for event in input_state.events {
            if let Some(InputEvent::Scroll { delta }) = event {
                let offset = self.y_offset as i64 - delta * (SCROLL_SPEED as i64);
                self.y_offset = i64::max(0, offset);
                redraw = true;
            }
        }

        if let Some(layout) = &self.layout {

            let PointerState { mut x, mut y, ..} = input_state.pointer;
            y = y - self.view_rect.y0 + self.y_offset;
            x = x - self.view_rect.x0;

            let hovered_rect = get_hovered_item(x, y, layout).map(|node| node.get_rect().clone());
            if hovered_rect != self.hovered_rect {
                redraw = true;
                self.hovered_rect = hovered_rect;
            }
        }

        redraw
    }

    pub fn draw(&mut self, fb: &mut Framebuffer) {
        if let Some(buffer) = self.buffer.as_ref() {

            let src_rect = {
                let mut r = buffer.shape_as_rect().clone();
                r.y0 += self.y_offset;
                r
            };

            fb.copy_from_fb(buffer, &src_rect, &self.view_rect, false);

            if let Some(mut r) = self.hovered_rect.clone() {
                r.y0 = r.y0 + self.view_rect.y0 - self.y_offset;
                r.x0 = r.x0 + self.view_rect.x0;
                blend_rect(fb, &r, Color::rgba(0, 0, 255, 128));
            }
        }
    }
}

fn draw_node(fb: &mut Framebuffer, node: &LayoutNode) {

    let &Rect { x0, y0, .. } = node.get_rect();

    if fb.w as i64 <= x0 || fb.h as i64 <= y0 {
        return;
    }

    match node {
        LayoutNode::Text { text, color, font, rect, .. } => {
            draw_str(fb, text, rect.x0, rect.y0, font, *color, None);
        },
        LayoutNode::Container { children, bg_color, rect, .. } => {

            if let &Some(bg_color) = bg_color {
                draw_rect(fb, &rect, bg_color);
            }

            for child in children.iter() {
                draw_node(fb, child);
            }
        }
    }
}

fn get_hovered_item<'a>(x: i64, y: i64, node: &'a LayoutNode) -> Option<&'a LayoutNode> {

    match node {
        LayoutNode::Text { .. } => None,
        // LayoutNode::Container { children, rect, .. } => match rect.check_contains_point(x, y) {
        //     false => None,
        //     true => {
        //         if let Some(child) = children.iter().find_map(|c| get_hovered_item(x, y, c)) {
        //             Some(child)
        //         } else {
        //             Some(node)
        //         }
        //     }
        // }
        LayoutNode::Container { children, rect, url, .. } => match rect.check_contains_point(x, y) {
            true => match url.is_some() {
                true => Some(node),
                false => children.iter().find_map(|c| get_hovered_item(x, y, c))
            },
            false => None
        }
    }

}

fn debug_layout(root_node: &LayoutNode) {

    fn repr_node(out_str: &mut String, node: &LayoutNode, depth: usize) {
        match node {
            LayoutNode::Text { text, rect, .. } => {
                out_str.push_str(&format!("{}{} {:?}\n"," ".repeat(depth), text, rect));
            },
            LayoutNode::Container { children, orientation, rect, .. } => {
                out_str.push_str(&format!("{}CONTAINER {:?} {:?}\n"," ".repeat(depth), orientation, rect));
                for child in children {
                    repr_node(out_str, child, depth+1);
                }
            }
        }
    }

    let mut out_str = String::new();
    repr_node(&mut out_str, root_node, 0);

    guestlib::print_console(&out_str);

}

enum LayoutNode {
    Text { text: String, color: Color, font: &'static Font, url: Option<String>, rect: Rect },
    Container { children: Vec<LayoutNode>, orientation: Orientation, bg_color: Option<Color>, rect: Rect, url: Option<String> }
}

impl LayoutNode {
    fn get_rect(&self) -> &Rect {
        match self {
            Self::Text { rect, .. } => rect,
            Self::Container { rect, .. } => rect,
        }
    }
}

#[derive(Debug, Clone, Copy)]
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

fn parse_html_to_layout(html: &str) -> LayoutNode {

    fn parse_node<'a>(node: ego_tree::NodeRef<'a, scraper::Node>, x0: i64, y0: i64) -> Option<LayoutNode> {

        match node.value() {

            scraper::Node::Element(element) if element.name() == "head" => None,

            scraper::Node::Element(element) => {

                let bg_color = match element.attr("bgcolor") {
                    Some(hex_str) => Some(parse_hexcolor(hex_str)),
                    _ => None
                };

                let url: Option<String> = match element.name() {
                    "a" => element.attr("href").map(|s| s.to_owned()),
                    _ => None,
                };

                let orientation = match element.name() {
                    "tr" => Orientation::Horizontal,
                    "tbody" => Orientation::Vertical,
                    "table" => Orientation::Vertical,
                    _ => Orientation::Horizontal
                };

                let mut children: Vec<LayoutNode> = Vec::new();
                let (mut child_x0, mut child_y0): (i64, i64) = (x0, y0);
                for html_child in node.children() {
                    if let Some(child_node) = parse_node(html_child, child_x0, child_y0) {
                        let &Rect { w: child_w, h: child_h, .. } = child_node.get_rect();
                        match orientation {
                            Orientation::Horizontal => child_x0 += child_w as i64,
                            Orientation::Vertical => child_y0 += child_h as i64,
                        }
                        children.push(child_node);
                    }
                }

                if children.len() > 0 {
                    let rect_0 = children[0].get_rect().clone();
                    let container_rect = children.iter()
                        .map(|c| c.get_rect().clone())
                        .fold(rect_0, |acc, r| r.bounding_box(&acc));
                    Some(LayoutNode::Container { children, orientation, bg_color, rect: container_rect, url })
                } else {
                    None
                }
            },

            scraper::Node::Text(text) if check_is_whitespace(&text) => None,

            scraper::Node::Text(text) => {

                //const M: Margins = Margins { left: 0, right: 0, top: 5, bottom: 5};
                const M: Margins = Margins { left: 0, right: 0, top: 0, bottom: 0};

                let text = core::str::from_utf8(text.as_bytes()).expect("Not UTF-8");
                let font = &HACK_15; // TODO
                let w = (text.len() * font.char_w) as u32 + M.left + M.right;
                let h = font.char_h as u32  + M.top + M.bottom;
                Some(LayoutNode::Text { 
                    text: text.to_string(),
                    color: Color::BLACK,  // TODO
                    font, 
                    url: None,
                    rect: Rect { 
                        x0: x0 + M.left as i64,
                        y0: y0 + M.top as i64,
                        w, h
                    }
                })
            },

            _ => None
        }
    }


    let tree = scraper::Html::parse_document(html).tree;

    let root = tree.root().first_child().expect("Empty HTML");

    parse_node(root, 0, 0).expect("Could not parse root HTML node")
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