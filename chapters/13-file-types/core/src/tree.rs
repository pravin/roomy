use std::ffi::OsString;
use std::ops::Index;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// What sort of thing a node is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    File,
    Dir,
    Symlink,
    Other,
}

/// A file or folder as the scanner finds it. Each entry owns its children,
/// so the whole thing is one big nested value. Easy to build, even on many
/// threads at once, but there's no way to walk back up from a child.
#[derive(Debug)]
pub struct Entry {
    pub name: OsString,
    pub kind: Kind,
    /// Bytes on disk. For a folder, everything inside it too.
    pub size: u64,
    /// How many files are in here, counting every level down.
    pub files: u64,
    /// We weren't allowed to look inside this folder.
    pub unreadable: bool,
    pub children: Vec<Entry>,
}

/// The number of a node in the tree. It's an index into a `Vec`, wrapped up
/// so it can't be mixed up with any other number.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct NodeId(pub u32);

impl NodeId {
    fn index(self) -> usize {
        self.0 as usize
    }
}

/// One file or folder in a `Tree`.
#[derive(Debug)]
pub struct Node {
    pub name: OsString,
    pub kind: Kind,
    pub size: u64,
    pub files: u64,
    pub unreadable: bool,
    pub parent: Option<NodeId>,
    /// Biggest first.
    pub children: Vec<NodeId>,
}

impl Node {
    pub fn display_name(&self) -> String {
        self.name.to_string_lossy().into_owned()
    }
}

/// Every file and folder from a scan, kept in one flat list. Nodes point at
/// each other by number, which lets a child know its parent without Rust
/// having to worry about who owns whom.
#[derive(Debug)]
pub struct Tree {
    root_path: PathBuf,
    nodes: Vec<Node>,
}

impl Tree {
    /// Flattens an `Entry` and everything in it into a tree. The root
    /// folder is node 0.
    pub fn from_entry(root_path: PathBuf, entry: Entry) -> Tree {
        let mut tree = Tree {
            root_path,
            nodes: Vec::new(),
        };
        tree.add(entry, None);
        tree
    }

    fn add(&mut self, mut entry: Entry, parent: Option<NodeId>) -> NodeId {
        let id = NodeId(self.nodes.len() as u32);
        entry.children.sort_by_key(|c| std::cmp::Reverse(c.size));
        let children = std::mem::take(&mut entry.children);
        self.nodes.push(Node {
            name: entry.name,
            kind: entry.kind,
            size: entry.size,
            files: entry.files,
            unreadable: entry.unreadable,
            parent,
            children: Vec::with_capacity(children.len()),
        });
        for child in children {
            let child_id = self.add(child, Some(id));
            self.nodes[id.index()].children.push(child_id);
        }
        id
    }

    pub fn root(&self) -> NodeId {
        NodeId(0)
    }

    pub fn root_path(&self) -> &Path {
        &self.root_path
    }

    pub fn get(&self, id: NodeId) -> Option<&Node> {
        self.nodes.get(id.index())
    }

    /// The full path of a node on disk.
    pub fn path(&self, id: NodeId) -> PathBuf {
        let mut path = self.root_path.clone();
        for ancestor in self.ancestors(id).into_iter().skip(1) {
            path.push(&self[ancestor].name);
        }
        path
    }

    /// The nodes from the root down to `id`, both included.
    pub fn ancestors(&self, id: NodeId) -> Vec<NodeId> {
        let mut chain = vec![id];
        let mut current = id;
        while let Some(parent) = self[current].parent {
            chain.push(parent);
            current = parent;
        }
        chain.reverse();
        chain
    }

    /// Every node below `id`, not including `id` itself.
    pub fn descendants(&self, id: NodeId) -> Descendants<'_> {
        Descendants {
            tree: self,
            stack: self[id].children.iter().rev().copied().collect(),
        }
    }
}

impl Index<NodeId> for Tree {
    type Output = Node;

    fn index(&self, id: NodeId) -> &Node {
        &self.nodes[id.index()]
    }
}

/// Walks a part of the tree, depth first. Made by `Tree::descendants`.
pub struct Descendants<'a> {
    tree: &'a Tree,
    stack: Vec<NodeId>,
}

impl Iterator for Descendants<'_> {
    type Item = NodeId;

    fn next(&mut self) -> Option<NodeId> {
        let id = self.stack.pop()?;
        self.stack
            .extend(self.tree[id].children.iter().rev().copied());
        Some(id)
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    pub fn file(name: &str, size: u64) -> Entry {
        Entry {
            name: name.into(),
            kind: Kind::File,
            size,
            files: 1,
            unreadable: false,
            children: Vec::new(),
        }
    }

    pub fn dir(name: &str, children: Vec<Entry>) -> Entry {
        Entry {
            name: name.into(),
            kind: Kind::Dir,
            size: children.iter().map(|c| c.size).sum(),
            files: children.iter().map(|c| c.files).sum(),
            unreadable: false,
            children,
        }
    }

    pub fn sample() -> Tree {
        Tree::from_entry(
            "/home/me".into(),
            dir(
                "/home/me",
                vec![
                    file("notes.txt", 10),
                    dir("photos", vec![file("a.jpg", 300), file("b.jpg", 200)]),
                    file("song.mp3", 50),
                ],
            ),
        )
    }

    #[test]
    fn children_are_biggest_first() {
        let tree = sample();
        let names: Vec<String> = tree[tree.root()]
            .children
            .iter()
            .map(|&id| tree[id].display_name())
            .collect();
        assert_eq!(names, ["photos", "song.mp3", "notes.txt"]);
    }

    #[test]
    fn paths_are_rebuilt_from_parents() {
        let tree = sample();
        let photos = tree[tree.root()].children[0];
        let a = tree[photos].children[0];
        assert_eq!(tree.path(a), Path::new("/home/me/photos/a.jpg"));
    }

    #[test]
    fn descendants_visits_everything_once() {
        let tree = sample();
        assert_eq!(tree.descendants(tree.root()).count(), 5);
    }
}
