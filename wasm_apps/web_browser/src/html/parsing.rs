use std::collections::BTreeMap;

use anyhow::anyhow;

use super::tree::Tree;

pub fn parse_html(html: &str) -> anyhow::Result<Tree<NodeData>> {
    let mut tree = Tree::new();
    let mut parent_id = None;

    for chunk in get_chunks(html) {
        match chunk.chunk_type {
            ChunkType::Text => {
                if tree.len() > 0 {
                    let text = html_escape::decode_html_entities(chunk.s).to_string();
                    let data = NodeData::Text { text };
                    tree.add_node(parent_id, data)?;
                }
            }

            ChunkType::Tag => match parse_tag(chunk.s)? {
                ParsedTag::Comment => (),

                ParsedTag::Open {
                    name,
                    attrs,
                    is_void,
                } => {
                    let data = NodeData::Tag { name, attrs };
                    let new_id = Some(tree.add_node(parent_id, data)?);
                    if !is_void {
                        parent_id = new_id;
                    }
                }

                ParsedTag::Close { name } => loop {
                    match parent_id {
                        None => {
                            log::warn!(
                                "Unexpected closing tag on line {} col {}: </{}> (no parent)",
                                chunk.line + 1,
                                chunk.col + 1,
                                name
                            );
                            break;
                        }

                        Some(p_id) => {
                            let curr_tag_name = match &tree.get_node(p_id).unwrap().data {
                                NodeData::Tag { name, .. } => name,
                                _ => {
                                    return Err(anyhow!(
                                        "line {} col {}: parent node is Text, should not happen",
                                        chunk.line + 1,
                                        chunk.col + 1
                                    ))
                                }
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
                },
            },
        }
    }

    Ok(tree)
}

fn check_is_void_element(tag_name: &str) -> bool {
    [
        "area", "base", "br", "col", "embed", "hr", "img", "input", "link", "meta", "param",
        "source", "track", "wbr",
    ]
    .contains(&tag_name)
}

#[derive(Debug)]
enum ParsedTag {
    Open {
        name: String,
        attrs: BTreeMap<String, String>,
        is_void: bool,
    },
    Close {
        name: String,
    },
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
        return Ok(ParsedTag::Comment);
    }

    let s = &s[1..s.len() - 1];

    let c2 = s.chars().next().unwrap();
    let (s, closing) = match c2 {
        '/' => (&s[1..], true),
        _ => (s, false),
    };

    let (name, s) = s.split_once(|c: char| c.is_whitespace()).unwrap_or((s, ""));
    let name = name.to_string();

    if closing {
        return Ok(ParsedTag::Close { name });
    }

    let attrs = parse_attrs(s)?;
    let is_void = check_is_void_element(&name);

    Ok(ParsedTag::Open {
        name,
        attrs,
        is_void,
    })
}

fn parse_attrs(s: &str) -> anyhow::Result<BTreeMap<String, String>> {
    #[derive(Debug, Clone, Copy)]
    enum State<'a> {
        Idle,
        InKey { i1: usize },
        InEqual { key: &'a str },
        InVal { key: &'a str, i1: usize },
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
            ('"', State::InEqual { key }) => State::InVal { key, i1: i + 1 },
            (_, State::InEqual { key }) => State::InEqual { key },

            ('"', State::InVal { key, i1 }) => {
                let val = s[i1..i].to_string();
                let key = html_escape::decode_html_entities(key).to_string();
                attrs.insert(key, val);
                State::Idle
            }
            (_, State::InVal { key, i1 }) => State::InVal { key, i1 },

            _ => return Err(anyhow!("Invalid attributes")),
        }
    }

    Ok(attrs)
}

fn get_chunks<'a>(html: &'a str) -> impl Iterator<Item = Chunk<'a>> {
    #[derive(Debug, Clone, Copy)]
    enum State {
        Idle,
        InTag { i1: usize, in_attr: bool },
        InText { i1: usize },
    }

    html.char_indices()
        .scan((State::Idle, 0, 0), |(state, line, col), (i, c)| {
            let mut new_chunk = None;

            if c == '\n' {
                *line += 1;
                *col = 0;
            } else {
                *col += 1;
            }

            *state = match (c, *state) {
                ('<', State::Idle) => State::InTag {
                    i1: i,
                    in_attr: false,
                },
                ('<', State::InText { i1 }) => {
                    let i2 = i;
                    new_chunk = Some(Chunk {
                        s: &html[i1..i2],
                        chunk_type: ChunkType::Text,
                        line: *line,
                        col: *col,
                    });
                    State::InTag {
                        i1: i,
                        in_attr: false,
                    }
                }

                ('>', State::InTag { i1, .. }) => {
                    let i2 = i + 1;
                    new_chunk = Some(Chunk {
                        s: &html[i1..i2],
                        chunk_type: ChunkType::Tag,
                        line: *line,
                        col: *col,
                    });
                    State::Idle
                }

                ('"', State::InTag { i1, in_attr }) => State::InTag {
                    i1,
                    in_attr: !in_attr,
                },

                (_, State::Idle) if c.is_whitespace() => State::Idle,

                (_, State::Idle) => State::InText { i1: i },
                (_, State::InTag { i1, in_attr }) => State::InTag { i1, in_attr },
                (_, State::InText { i1 }) => State::InText { i1 },
            };

            Some(new_chunk)
        })
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

#[derive(Debug)]
pub enum NodeData {
    Tag {
        name: String,
        attrs: BTreeMap<String, String>,
    },
    Text {
        text: String,
    },
}

