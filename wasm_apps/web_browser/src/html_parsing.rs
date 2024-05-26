use std::collections::BTreeMap;
use std::error::Error;
use std::result::Result;
use std::fmt;

pub fn parse_html(html: &str) -> Result<HtmlTree, HtmlError> {

    let mut tree = HtmlTree::new();
    let mut parent_id = None;

    for chunk in get_chunks(html) {

        match chunk.chunk_type {

            ChunkType::Text => {
                if tree.len() > 0 {
                    let data = NodeData::Text { text: chunk.s };
                    tree.add_node(parent_id, data)?;
                }
            },

            ChunkType::Tag => match parse_tag(chunk.s)? {

                ParsedTag::Open { name, attrs, is_void } => {
                    let data = NodeData::Tag { name, attrs };
                    let new_id = Some(tree.add_node(parent_id, data)?);
                    if !is_void { parent_id = new_id; }
                },

                ParsedTag::Close { name } => {

                    let curr_tag_name = parent_id
                        .map(|parent_id| match tree.get_node(parent_id).unwrap().data {
                            NodeData::Tag { name, .. } => Some(name),
                            _ => None,
                        })
                        .flatten();

                    if let Some(curr_tag_name) = curr_tag_name {
                        if name != curr_tag_name {
                            return Err(HtmlError::new(&format!("Unexpected closing tag: {}", name)))
                        }
                    }

                    parent_id = tree.get_parent(parent_id.unwrap())?;
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
enum ParsedTag<'a> {
    Open { name: &'a str, attrs: BTreeMap<&'a str, &'a str>, is_void: bool },
    Close { name: &'a str }
}

fn parse_tag(s: &str) -> Result<ParsedTag, HtmlError> {

    if s.len() < 3 {
        return Err(HtmlError::new("Invalid tag"));
    }

    let c0 = s.chars().next().unwrap();
    let c1 = s.chars().next_back().unwrap();

    if c0 != '<' || c1 != '>' {
        return Err(HtmlError::new("Missing < >"));
    }

    let s = &s[1..s.len()-1];

    let c2 = s.chars().next().unwrap();
    let (s, closing) = match c2 {
        '/' => (&s[1..], true),
        _ => (s, false)
    };

    let (name, s) = s.split_once(' ').unwrap_or((s, ""));

    if closing {
        return Ok(ParsedTag::Close { name })
    }

    let attrs = parse_attrs(s)?;
    let is_void = check_is_void_element(name);

    Ok(ParsedTag::Open { name, attrs, is_void })

}

fn parse_attrs(s: &str) -> Result<BTreeMap<&str, &str>, HtmlError> {

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
                let val = &s[i1..i];
                attrs.insert(key, val);
                State::Idle
            },
            (_, State::InVal { key, i1 }) => State::InVal { key, i1 },

            _ => return Err(HtmlError::new("Invalid attributes"))
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
        State::Idle,
        |state, (i, c)| {

            let mut new_chunk = None;

            *state = match (c, *state) {
                ('<', State::Idle) => State::InTag { i1: i, in_attr: false },
                ('<', State::InText { i1 }) => {
                    let i2 = i;
                    new_chunk = Some(Chunk { s: &html[i1..i2], chunk_type: ChunkType::Text });
                    State::InTag { i1: i, in_attr: false }
                },

                ('>', State::InTag { i1, .. }) => {
                    let i2 = i + 1;
                    new_chunk = Some(Chunk { s: &html[i1..i2], chunk_type: ChunkType::Tag });
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
}

#[derive(Debug)]
enum ChunkType {
    Tag,
    Text,
}


#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NodeId(pub usize);

pub struct Node<'a> {
    pub data: NodeData<'a>,
    pub parent: Option<NodeId>,
    pub children: Vec<NodeId>,
}


#[derive(Debug)]
pub enum NodeData<'a> {
    Tag { name: &'a str, attrs: BTreeMap<&'a str, &'a str> },
    Text { text: &'a str }
}


pub struct HtmlTree<'a> {
    nodes: Vec<Node<'a>>
}

#[derive(Debug)]
pub struct HtmlError { msg: String }

impl fmt::Display for HtmlError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "TreeError")
    }
}

impl Error for HtmlError {}

impl HtmlError {
    fn new(msg: &str) -> Self {
        Self { msg: msg.to_owned() }
    }
}

impl<'a> HtmlTree<'a> {

    fn new() -> Self {
        Self { nodes: vec![] }
    }

    pub fn get_node(&self, node_id: NodeId) -> Option<&Node<'a>> {
        self.nodes.get(node_id.0)
    }

    fn get_node_mut(&mut self, node_id: NodeId) -> Option<&mut NodeData<'a>> {
        self.nodes.get_mut(node_id.0).map(|node| &mut node.data)
    }

    fn add_node(&mut self, parent_id: Option<NodeId>, data: NodeData<'a>) -> Result<NodeId, HtmlError> {

        let child_id = NodeId(self.nodes.len());

        let mut child = Node { data, parent: None, children: Vec::new() };

        if let Some(parent_id) = parent_id {
            let parent_node = self.nodes.get_mut(parent_id.0).ok_or(HtmlError::new("No such parent ID"))?;
            parent_node.children.push(child_id);
            child.parent = Some(parent_id);
        } else {
            if child_id != NodeId(0) {
                return Err(HtmlError::new("Tree already has a root node"));
            }
            child.parent = None;
        }

        self.nodes.push(child);

        Ok(child_id)
    }

    fn get_parent(&self, node_id: NodeId) -> Result<Option<NodeId>, HtmlError> {
        let parent_node = self.nodes.get(node_id.0).ok_or(HtmlError::new("No such parent ID"))?;
        Ok(parent_node.parent)
    }

    fn len(&self) -> usize {
        self.nodes.len()
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
                NodeData::Tag { name, attrs } => {
    
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