use applib::{drawing::text::{RichText, DEFAULT_FONT_FAMILY, Font}, Color, FbData};
use super::tree::{Tree, NodeId};
use super::parsing::HtmlNode;


pub fn compute_block_layout(html_tree: &Tree<HtmlNode>) -> Tree<Block> {

    let mut layout_tree = Tree::new();
    let layout_root_id = layout_tree.add_node(
        None,
        Block::Container { color: None, orientation: Orientation::Vertical }
    ).unwrap();


    parse_block_tag(
        html_tree,
        NodeId(0),
        &mut layout_tree,
        layout_root_id
    );

    layout_tree
}

pub fn parse_block_tag(

    html_tree: &Tree<HtmlNode>,
    html_id: NodeId,

    layout_tree: &mut Tree<Block>,
    layout_id: NodeId,
) {

    let html_node = html_tree.get_node(html_id).unwrap();

    let mut curr_layout_child_id = None;

    for html_child_id in html_node.children.iter() {

        let html_child_node = html_tree.get_node(*html_child_id).unwrap();

        match &html_child_node.data {

            HtmlNode::Tag { name, attrs, .. } => match get_element_type(name) {
                ElementType::Skipped => {},
                ElementType::Unknown => log::debug!("Unknown HTML tag <{}>", name),
                ElementType::Linebreak => curr_layout_child_id = None,
                ElementType::Image => {
                    let w: u32 = attrs.get("width").map(|s| s.parse().ok()).flatten().unwrap_or(0);
                    let h: u32 = attrs.get("height").map(|s| s.parse().ok()).flatten().unwrap_or(0);
                    layout_tree.add_node(
                        Some(layout_id),
                        Block::Image { w, h },
                    ).unwrap();

                    curr_layout_child_id = None;
                },
                ElementType::Block { orientation } => {
                    let color = match attrs.get("bgcolor") {
                        Some(hex_str) => Some(parse_hexcolor(hex_str)),
                        _ => None,
                    };
    
                    let node_id = layout_tree.add_node(
                        Some(layout_id),
                        Block::Container { color, orientation },
                    ).unwrap();
    
                    parse_block_tag(html_tree, *html_child_id, layout_tree, node_id);
    
                    curr_layout_child_id = None;
                },
                ElementType::Inline => {
                    let new_text = get_inline_block_contents(html_tree, *html_child_id);

                    let node_id = curr_layout_child_id.get_or_insert_with(|| layout_tree.add_node(
                        Some(layout_id),
                        Block::Text { text: RichText::new() }
                    ).unwrap());
    
                    match layout_tree.get_node_data_mut(*node_id).unwrap() {
                        Block::Text { text, .. } => text.concat(new_text),
                        _ => unreachable!()
                    };
                }
            },

            HtmlNode::Text { .. } => {
                let new_text = get_inline_block_contents(html_tree, *html_child_id);

                let node_id = curr_layout_child_id.get_or_insert_with(|| layout_tree.add_node(
                    Some(layout_id),
                    Block::Text { text: RichText::new() }
                ).unwrap());

                match layout_tree.get_node_data_mut(*node_id).unwrap() {
                    Block::Text { text, .. } => text.concat(new_text),
                    _ => unreachable!()
                };
            }
        }
    }

}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Orientation {
    Horizontal,
    Vertical,
}

#[derive(PartialEq)]
enum ElementType {
    Block { orientation: Orientation },
    Inline,
    Skipped,
    Linebreak,
    Unknown,
    Image,
}

fn get_element_type(tag_name: &str) -> ElementType {

    let SKIPPED = [
        "head",
        "script",
        "font",
    ];

    let VERTICAL_BLOCK = [
        "html",
        "body",
        "p",
        "div",
        "center",

        "table",
        "td",
    ];

    let HORIZONTAL_BLOCK = [
        "tr",
    ];

    let INLINE = [
        "span",
        "h1",
        "h2",
        "h3",
        "strong",
        "a",
        "b",
        "i",
        "u",
    ];

    let IMAGE = [
        "img"
    ];

    if SKIPPED.contains(&tag_name) { ElementType::Skipped }
    else if VERTICAL_BLOCK.contains(&tag_name) { ElementType::Block { orientation: Orientation::Vertical } }
    else if HORIZONTAL_BLOCK.contains(&tag_name) { ElementType::Block { orientation: Orientation::Horizontal } }
    else if INLINE.contains(&tag_name) { ElementType::Inline }
    else if IMAGE.contains(&tag_name) { ElementType::Image }
    else if tag_name == "br" { ElementType::Linebreak }
    else { ElementType::Unknown }
}

fn parse_hexcolor(hex_str: &str) -> Color {
    let mut color_bytes = hex::decode(hex_str.replace("#", "")).expect("Invalid color");

    match color_bytes.len() {
        3 => color_bytes.push(255),
        4 => (),
        _ => panic!("Invalid color: {:?}", color_bytes),
    };

    let color_bytes: [u8; 4] = color_bytes.try_into().unwrap();

    Color(color_bytes)
}

fn get_inline_block_contents(html_tree: &Tree<HtmlNode>, html_id: NodeId) -> RichText {

    let mut inline_text = RichText::new();

    #[derive(Clone)]
    struct TextContext { color: Color, font: &'static Font }

    fn get_contents(html_tree: &Tree<HtmlNode>, html_id: NodeId, context: &TextContext, inline_text: &mut RichText) {

        let html_child_node = html_tree.get_node(html_id).unwrap();

        match &html_child_node.data {
            HtmlNode::Tag { name, attrs, .. } => match get_element_type(name) {
                ElementType::Inline => {
                    let mut context = context.clone();
                    context.color = {
                        if name == "a" { Color::BLUE }
                        else if let Some(color) = attrs.get("color").map(|s| parse_hexcolor(s)) { color }
                        else { context.color }
                    };
    
                    for child_id in html_tree.get_node(html_id).unwrap().children.iter() {
                        get_contents(html_tree, *child_id, &context, inline_text);
                    }
                },
                _ => log::warn!("Found block tag <{}> inside of an inline tag, skipping", name),
            },

            HtmlNode::Text { text } => {
                inline_text.concat(RichText::from_str(text, context.color, context.font));
            }
        }
    }

    let context = TextContext { color: Color::BLACK, font: DEFAULT_FONT_FAMILY.get_default() };
    get_contents(html_tree, html_id, &context, &mut inline_text);

    inline_text
}

#[derive(Debug)]
pub enum Block {
    Container { color: Option<Color>, orientation: Orientation },
    Text { text: RichText },
    Image { w: u32, h: u32 },
}

