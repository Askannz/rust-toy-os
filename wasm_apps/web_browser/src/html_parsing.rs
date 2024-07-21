use std::collections::BTreeMap;

use anyhow::anyhow;

pub fn parse_html(html: &str) -> anyhow::Result<HtmlTree> {

    let mut tree = HtmlTree::new();
    let mut parent_id = None;

    for chunk in get_chunks(html) {

        match chunk.chunk_type {

            ChunkType::Text => {
                if tree.len() > 0 {
                    let text = html_escape::decode_html_entities(chunk.s).to_string();
                    let data = NodeData::Text { text };
                    tree.add_node(parent_id, data)?;
                }
            },

            ChunkType::Tag => match parse_tag(chunk.s)? {

                ParsedTag::Comment => (),

                ParsedTag::Open { name, attrs, is_void } => {
                    let data = NodeData::Tag { name, attrs };
                    let new_id = Some(tree.add_node(parent_id, data)?);
                    if !is_void { parent_id = new_id; }
                },

                ParsedTag::Close { name } => loop {
                    
                    match parent_id {
    
                        None => {
                            log::warn!(
                                "Unexpected closing tag on line {} col {}: </{}> (no parent)",
                                chunk.line+1, chunk.col+1, name
                            );
                            break;
                        },

                        Some(p_id) => {

                            let curr_tag_name = match &tree.get_node(p_id).unwrap().data {
                                NodeData::Tag { name, .. } => name,
                                _ => return Err(anyhow!(
                                        "line {} col {}: parent node is Text, should not happen",
                                        chunk.line+1, chunk.col+1
                                    )),
                            };


                            if &name == curr_tag_name {
                                parent_id = tree.get_parent(p_id)?;
                                break;
                            } else {
                                log::warn!(
                                    "Unexpected closing tag on line {} col {}: </{}>. Discarding parent <{}>.",
                                    chunk.line+1, chunk.col+1, name, curr_tag_name
                                );

                                let parent_parent_id = tree.get_parent(p_id)?;
                                if let Some(p_p_id) = parent_parent_id {
                                    tree.transfer_children(p_id, p_p_id);
                                }
                                parent_id = parent_parent_id;
                            }
                        }
                    }

                }
            }
        }
    }

    Ok(tree)
}

fn check_is_void_element(tag_name: &str) -> bool {
    [
        "area",
        "base",
        "br",
        "col",
        "embed",
        "hr",
        "img",
        "input",
        "link",
        "meta",
        "param",
        "source",
        "track",
        "wbr",
    ]
    .contains(&tag_name)
}

#[derive(Debug)]
enum ParsedTag {
    Open { name: String, attrs: BTreeMap<String, String>, is_void: bool },
    Close { name: String },
    Comment,
}

fn parse_tag(s: &str) -> anyhow::Result<ParsedTag> {

    if s.len() < 3 {
        return Err(anyhow!("Invalid tag"));
    }

    let c0 = s.chars().next().unwrap();
    let c1 = s.chars().next_back().unwrap();

    if c0 != '<' || c1 != '>' {
        return Err(anyhow!("Missing < >"));
    }

    if s.starts_with("<!--") || s.ends_with("-->") {
        return  Ok(ParsedTag::Comment);
    }

    let s = &s[1..s.len()-1];

    let c2 = s.chars().next().unwrap();
    let (s, closing) = match c2 {
        '/' => (&s[1..], true),
        _ => (s, false)
    };

    let (name, s) = s.split_once(|c: char| c.is_whitespace()).unwrap_or((s, ""));
    let name = name.to_string();

    if closing {
        return Ok(ParsedTag::Close { name })
    }

    let attrs = parse_attrs(s)?;
    let is_void = check_is_void_element(&name);

    Ok(ParsedTag::Open { name, attrs, is_void })

}

fn parse_attrs(s: &str) -> anyhow::Result<BTreeMap<String, String>> {

    #[derive(Debug, Clone, Copy)]
    enum State<'a> { 
        Idle,
        InKey { i1: usize},
        InEqual { key: &'a str },
        InVal { key: &'a str, i1: usize }
    }

    let mut state = State::Idle;
    let mut attrs = BTreeMap::new();

    for (i, c) in s.char_indices() {
        state = match (c, state) {

            (c, State::Idle) if c.is_whitespace() => State::Idle,
            (c, State::Idle) if !c.is_whitespace() => State::InKey { i1: i },

            (c, State::InKey { i1 }) if c.is_whitespace() => State::InEqual { key: &s[i1..i] },
            ('=', State::InKey { i1 }) => State::InEqual { key: &s[i1..i] },
            (_, State::InKey { i1 }) => State::InKey { i1 },

            ('=', State::InEqual { key }) => State::InEqual { key },
            ('"', State::InEqual { key }) => State::InVal { key, i1: i+1 },
            (_, State::InEqual { key }) => State::InEqual { key },

            ('"', State::InVal { key, i1 }) => {
                let val = s[i1..i].to_string();
                let key = html_escape::decode_html_entities(key).to_string();
                attrs.insert(key, val);
                State::Idle
            },
            (_, State::InVal { key, i1 }) => State::InVal { key, i1 },

            _ => return Err(anyhow!("Invalid attributes"))
        }
    }

    Ok(attrs)
}


fn get_chunks<'a>(html: &'a str) -> impl Iterator<Item = Chunk<'a>> {

    #[derive(Debug, Clone, Copy)]
    enum State { 
        Idle,
        InTag { i1: usize, in_attr: bool },
        InText { i1: usize }
    }

    html.char_indices().scan(
        (State::Idle, 0, 0),
        |(state, line, col), (i, c)| {

            let mut new_chunk = None;

            if c == '\n' {
                *line += 1;
                *col = 0;
            } else {
                *col += 1;
            }

            *state = match (c, *state) {
                ('<', State::Idle) => State::InTag { i1: i, in_attr: false },
                ('<', State::InText { i1 }) => {
                    let i2 = i;
                    new_chunk = Some(Chunk { s: &html[i1..i2], chunk_type: ChunkType::Text, line: *line, col: *col });
                    State::InTag { i1: i, in_attr: false }
                },

                ('>', State::InTag { i1, .. }) => {
                    let i2 = i + 1;
                    new_chunk = Some(Chunk { s: &html[i1..i2], chunk_type: ChunkType::Tag, line: *line, col: *col });
                    State::Idle
                },

                ('"', State::InTag { i1, in_attr }) => {
                    State::InTag { i1, in_attr: !in_attr }
                }

                (_, State::Idle) if c.is_whitespace() => State::Idle,

                (_, State::Idle) => State::InText { i1: i },
                (_, State::InTag { i1, in_attr }) => State::InTag { i1, in_attr },
                (_, State::InText { i1 }) => State::InText { i1 },
            };

            Some(new_chunk)
        }
    )
    .filter_map(|chunk| chunk)
}

#[derive(Debug)]
struct Chunk<'a> {
    s: &'a str,
    chunk_type: ChunkType,
    line: usize,
    col: usize,
}

#[derive(Debug)]
enum ChunkType {
    Tag,
    Text,
}


#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NodeId(pub usize);

pub struct Node {
    pub data: NodeData,
    pub parent: Option<NodeId>,
    pub children: Vec<NodeId>,
}


#[derive(Debug)]
pub enum NodeData {
    Tag { name: String, attrs: BTreeMap<String, String> },
    Text { text: String }
}


pub struct HtmlTree {
    nodes: Vec<Node>
}



impl HtmlTree {

    fn new() -> Self {
        Self { nodes: vec![] }
    }

    pub fn get_node(&self, node_id: NodeId) -> Option<&Node> {
        self.nodes.get(node_id.0)
    }

    fn get_node_mut(&mut self, node_id: NodeId) -> Option<&mut NodeData> {
        self.nodes.get_mut(node_id.0).map(|node| &mut node.data)
    }

    fn add_node(&mut self, parent_id: Option<NodeId>, data: NodeData) -> anyhow::Result<NodeId> {

        let child_id = NodeId(self.nodes.len());

        let mut child = Node { data, parent: None, children: Vec::new() };

        if let Some(parent_id) = parent_id {
            let parent_node = self.nodes.get_mut(parent_id.0).ok_or(anyhow!("No such parent ID"))?;
            parent_node.children.push(child_id);
            child.parent = Some(parent_id);
        } else {
            if child_id != NodeId(0) {
                return Err(anyhow!("Tree already has a root node"));
            }
            child.parent = None;
        }

        self.nodes.push(child);

        Ok(child_id)
    }

    fn get_parent(&self, node_id: NodeId) -> anyhow::Result<Option<NodeId>> {
        let parent_node = self.nodes.get(node_id.0).ok_or(anyhow!("No such parent ID"))?;
        Ok(parent_node.parent)
    }

    fn len(&self) -> usize {
        self.nodes.len()
    }

    fn transfer_children(&mut self, src_id: NodeId, dst_id: NodeId) {

        let (src_node, dst_node) = {
            assert!(src_id != dst_id);
            let (id_1, id_2) = if src_id.0 < dst_id.0 { (src_id.0, dst_id.0) } else { (dst_id.0, src_id.0) };
            let (node_1, tail) = self.nodes[id_1..].split_first_mut().expect("Tree is empty");
            let node_2 = &mut tail[id_2 - id_1 - 1];
            if src_id.0 < dst_id.0 { (node_1, node_2) } else { (node_2, node_1) }
        };

        dst_node.children.extend(src_node.children.iter());
        src_node.children.clear();
    }

    fn plot(&self) -> String {

        fn repr_node(tree: &HtmlTree, out_str: &mut String, node_id: NodeId, is_last: bool, prefix: &str) {

            let c = match is_last {
                true => "└",
                false => "├",
            };

            let node = tree.get_node(node_id).unwrap();
    
            match &node.data {
                NodeData::Text { text } => {
                    for line in text.split("\n") {
                        out_str.push_str(&format!("{}{}{}\n", prefix, c, line));
                    }
                },
                NodeData::Tag { name, attrs, .. } => {
    
                    out_str.push_str(&format!("{}{}{} {:?}\n", prefix, c, name, attrs));
    
                    let c2 = match is_last {
                        true => " ",
                        false => "|",
                    };
    
                    let child_prefix = format!("{}{}", prefix, c2);
    
                    for (i, child_id) in node.children.iter().enumerate() {
                        let child_is_last = i == node.children.len() - 1;
                        repr_node(tree, out_str, *child_id, child_is_last, &child_prefix);
                    }
                }
            }
        }

        let root_node_id = NodeId(0);
    
        let mut out_str = String::new();
        repr_node(self, &mut out_str, root_node_id, false, "");
    
        out_str
    }
}