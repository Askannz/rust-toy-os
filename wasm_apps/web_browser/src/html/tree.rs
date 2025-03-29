use alloc::fmt::Debug;
use anyhow::anyhow;

pub struct Tree<T: Debug> {
    nodes: Vec<Node<T>>,
}

pub struct Node<T: Debug> {
    pub data: T,
    pub parent: Option<NodeId>,
    pub children: Vec<NodeId>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NodeId(pub usize);

impl<T: Debug> Tree<T> {
    pub fn new() -> Self {
        Self { nodes: vec![] }
    }

    pub fn get_node(&self, node_id: NodeId) -> Option<&Node<T>> {
        self.nodes.get(node_id.0)
    }

    pub fn get_node_mut(&mut self, node_id: NodeId) -> Option<&mut T> {
        self.nodes.get_mut(node_id.0).map(|node| &mut node.data)
    }

    pub fn add_node(&mut self, parent_id: Option<NodeId>, data: T) -> anyhow::Result<NodeId> {
        let child_id = NodeId(self.nodes.len());

        let mut child = Node {
            data,
            parent: None,
            children: Vec::new(),
        };

        if let Some(parent_id) = parent_id {
            let parent_node = self
                .nodes
                .get_mut(parent_id.0)
                .ok_or(anyhow!("No such parent ID"))?;
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

    pub fn get_parent(&self, node_id: NodeId) -> anyhow::Result<Option<NodeId>> {
        let parent_node = self
            .nodes
            .get(node_id.0)
            .ok_or(anyhow!("No such parent ID"))?;
        Ok(parent_node.parent)
    }

    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    pub fn transfer_children(&mut self, src_id: NodeId, dst_id: NodeId) {
        let (src_node, dst_node) = {
            assert!(src_id != dst_id);
            let (id_1, id_2) = if src_id.0 < dst_id.0 {
                (src_id.0, dst_id.0)
            } else {
                (dst_id.0, src_id.0)
            };
            let (node_1, tail) = self.nodes[id_1..].split_first_mut().expect("Tree is empty");
            let node_2 = &mut tail[id_2 - id_1 - 1];
            if src_id.0 < dst_id.0 {
                (node_1, node_2)
            } else {
                (node_2, node_1)
            }
        };

        dst_node.children.extend(src_node.children.iter());
        src_node.children.clear();
    }

    pub fn plot(&self) -> String {
        fn repr_node<T: Debug>(
            tree: &Tree<T>,
            out_str: &mut String,
            node_id: NodeId,
            is_last: bool,
            prefix: &str,
        ) {
            let c = match is_last {
                true => "└",
                false => "├",
            };

            let node = tree.get_node(node_id).unwrap();

            out_str.push_str(&format!("{}{}{:?}\n", prefix, c, node.data));

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

        let root_node_id = NodeId(0);

        let mut out_str = String::new();
        repr_node(self, &mut out_str, root_node_id, false, "");

        out_str
    }
}
