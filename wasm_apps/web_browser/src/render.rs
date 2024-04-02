use applib::{Color, Rect, Framebuffer};
use applib::drawing::primitives::draw_rect;
use applib::drawing::text::draw_str;
use applib::drawing::text::{HACK_15, Font};
use applib::input::{InputState, InputEvent};


pub struct Webview<'a> {
    buffer: Option<Framebuffer<'a>>,
    view_rect: Rect,
    y_offset: i64,
}

const SCROLL_SPEED: u32 = 10;

impl<'a> Webview<'a> {

    pub fn new(view_rect: &Rect) -> Self {
        Self { buffer: None, view_rect: view_rect.clone(), y_offset: 0 }
    }

    pub fn update(&mut self, input_state: &InputState, html_update: Option<&str>) -> bool {

        let mut redraw = false;

        if let Some(html) = html_update {

            let root_node = parse_html_to_layout(html);

            let bw = get_node_width(&root_node);
            let bh = get_node_height(&root_node);
    
            let mut buffer = Framebuffer::new_owned(bw, bh);
    
            draw_node(&mut buffer, 0, 0, &root_node);
    
            self.buffer = Some(buffer);

            redraw = true;
        }

        for event in input_state.events {
            if let Some(InputEvent::Scroll { delta }) = event {
                let offset = self.y_offset as i64 - delta * (SCROLL_SPEED as i64);
                self.y_offset = i64::max(0, offset);
                redraw = true;
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
        }
    }
}


fn get_node_height(node: &RenderNode) -> u32 {
    match node {
        RenderNode::Text { font, .. } => font.char_h as u32,
        RenderNode::Container { children, orientation, .. } => {

            let children_heights = children.iter().map(get_node_height);

            match orientation {
                Orientation::Horizontal => children_heights.max().unwrap_or(0),
                Orientation::Vertical => children_heights.sum(),
            }
        }
    }
}

fn get_node_width(node: &RenderNode) -> u32 {
    match node {
        RenderNode::Text { text, font, .. } => (text.len() * font.char_w) as u32,
        RenderNode::Container { children, orientation, .. } => {

            let children_widths = children.iter().map(get_node_width);

            match orientation {
                Orientation::Horizontal => children_widths.sum(),
                Orientation::Vertical => children_widths.max().unwrap_or(0),
            }
        }
    }
}

fn draw_node(fb: &mut Framebuffer, x0: i64, y0: i64, node: &RenderNode) {

    if fb.w as i64 <= x0 || fb.h as i64 <= y0 {
        return;
    }

    match node {
        RenderNode::Text { text, color, font } => {
            draw_str(fb, text, x0, y0, font, *color, None);
        },
        RenderNode::Container { children, orientation, bg_color } => {

            if let &Some(bg_color) = bg_color {
                let node_w = get_node_width(node);
                let node_h = get_node_height(node);
                let rect = Rect { x0, y0, w: node_w, h: node_h  };
                draw_rect(fb, &rect, bg_color);
            }

            let (mut child_x0, mut child_y0): (i64, i64) = (x0, y0);
            for child in children.iter() {
                draw_node(fb, child_x0, child_y0, child);
                match orientation {
                    Orientation::Horizontal => { 
                        let child_w: i64 = get_node_width(child).into();
                        child_x0 += child_w;
                    },
                    Orientation::Vertical => { 
                        let child_h: i64 = get_node_height(child).into();
                        child_y0 += child_h;
                    },
                }
            }
        }
    }
}


pub fn render_html(fb: &mut Framebuffer, html: &str) {




    let root_node = parse_html_to_layout(html);

    //debug_layout(&root_node);

    draw_node(fb, 0, 0, &root_node);

}

fn debug_layout(root_node: &RenderNode) {

    fn repr_node(out_str: &mut String, node: &RenderNode, depth: usize) {
        match node {
            RenderNode::Text { text, .. } => {
                out_str.push_str(&format!("{}{}\n"," ".repeat(depth), text));
            },
            RenderNode::Container { children, orientation, .. } => {
                out_str.push_str(&format!("{}CONTAINER {:?}\n"," ".repeat(depth), orientation));
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

enum RenderNode {
    Text { text: String, color: Color, font: &'static Font },
    Container { children: Vec<RenderNode>, orientation: Orientation, bg_color: Option<Color> }
}

#[derive(Debug, Clone, Copy)]
enum Orientation {
    Horizontal,
    Vertical,
}


fn parse_html_to_layout(html: &str) -> RenderNode {


    fn parse_node<'a>(node: ego_tree::NodeRef<'a, scraper::Node>) -> Option<RenderNode> {

        match node.value() {

            scraper::Node::Element(element) if element.name() == "head" => None,

            scraper::Node::Element(element) => {

                let bg_color = match element.attr("bgcolor") {
                    Some(hex_str) => Some(parse_hexcolor(hex_str)),
                    _ => None
                };

                let children: Vec<RenderNode> = node.children().filter_map(parse_node).collect();

                let orientation = match element.name() {
                    "tr" => Orientation::Horizontal,
                    "tbody" => Orientation::Vertical,
                    "table" => Orientation::Vertical,
                    _ => Orientation::Horizontal
                };

                Some(RenderNode::Container { children, orientation, bg_color })
            },

            scraper::Node::Text(text) if check_is_whitespace(&text) => None,

            scraper::Node::Text(text) => {
                let text = core::str::from_utf8(text.as_bytes()).expect("Not UTF-8");
                Some(RenderNode::Text { 
                    text: text.to_string(),
                    color: Color::BLACK,  // TODO
                    font: &HACK_15, // TODO
                })
            },

            _ => None
        }
    }


    let tree = scraper::Html::parse_document(html).tree;

    let root = tree.root().first_child().expect("Empty HTML");

    parse_node(root).expect("Could not parse root HTML node")
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