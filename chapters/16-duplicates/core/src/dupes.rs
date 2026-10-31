//! Finding files that are exact copies of each other.
//!
//! Reading every byte of every file would take forever, so we narrow things
//! down in three passes, each slower than the last but run on fewer files:
//! same size, then same first 64 KB, then same everything.

use std::collections::HashMap;
use std::fs::File;
use std::hash::Hash;
use std::io::{self, Read};
use std::path::Path;

use rayon::prelude::*;
use serde::Serialize;

use crate::tree::{Kind, NodeId, Tree};

/// Files that are byte for byte the same.
#[derive(Clone, Debug, Serialize)]
pub struct DuplicateGroup {
    /// The size of one copy.
    pub size: u64,
    pub ids: Vec<NodeId>,
}

impl DuplicateGroup {
    /// Space you'd get back by keeping only one copy.
    pub fn wasted(&self) -> u64 {
        self.size * (self.ids.len() as u64 - 1)
    }
}

/// Finds duplicate files below `start` that are at least `min_size` bytes,
/// most wasted space first.
pub fn find(tree: &Tree, start: NodeId, min_size: u64) -> Vec<DuplicateGroup> {
    // Pass 1: group by size. No reading needed, the scan already told us.
    let mut by_size: HashMap<u64, Vec<NodeId>> = HashMap::new();
    for id in tree.descendants(start) {
        let node = &tree[id];
        if node.kind == Kind::File && node.size >= min_size.max(1) {
            by_size.entry(node.size).or_default().push(id);
        }
    }
    let groups: Vec<Vec<NodeId>> = by_size.into_values().filter(|g| g.len() > 1).collect();

    // Pass 2: the first 64 KB. Pass 3: the whole file.
    let groups = refine(groups, |id| hash_start(&tree.path(id)).ok());
    let groups = refine(groups, |id| hash_all(&tree.path(id)).ok());

    let mut found: Vec<DuplicateGroup> = groups
        .into_iter()
        .map(|ids| DuplicateGroup {
            size: tree[ids[0]].size,
            ids,
        })
        .collect();
    found.sort_by_key(|g| std::cmp::Reverse(g.wasted()));
    found
}

/// Splits each group by a key, on every core, keeping only the pieces that
/// still have more than one file in them. Files we can't read drop out.
fn refine<K, F>(groups: Vec<Vec<NodeId>>, key: F) -> Vec<Vec<NodeId>>
where
    K: Eq + Hash + Send,
    F: Fn(NodeId) -> Option<K> + Sync,
{
    groups
        .into_par_iter()
        .flat_map_iter(|group| {
            let keyed: Vec<(K, NodeId)> = group
                .par_iter()
                .filter_map(|&id| Some((key(id)?, id)))
                .collect();
            let mut split: HashMap<K, Vec<NodeId>> = HashMap::new();
            for (k, id) in keyed {
                split.entry(k).or_default().push(id);
            }
            split.into_values().filter(|g| g.len() > 1)
        })
        .collect()
}

fn hash_start(path: &Path) -> io::Result<blake3::Hash> {
    let mut buffer = vec![0; 64 * 1024];
    let mut file = File::open(path)?;
    let mut filled = 0;
    // `read` can return less than we asked for, so keep going until the
    // buffer is full or the file runs out.
    loop {
        let n = file.read(&mut buffer[filled..])?;
        if n == 0 || filled + n == buffer.len() {
            filled += n;
            break;
        }
        filled += n;
    }
    Ok(blake3::hash(&buffer[..filled]))
}

fn hash_all(path: &Path) -> io::Result<blake3::Hash> {
    let mut hasher = blake3::Hasher::new();
    hasher.update_reader(File::open(path)?)?;
    Ok(hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Progress, ScanOptions, scan};
    use std::fs;

    #[test]
    fn finds_copies_and_ignores_lookalikes() {
        let dir = tempfile::tempdir().unwrap();
        let photo: Vec<u8> = (0..200_000u32).map(|i| (i % 251) as u8).collect();
        let mut almost = photo.clone();
        *almost.last_mut().unwrap() ^= 1; // Same size, same start, different end

        fs::create_dir(dir.path().join("backup")).unwrap();
        fs::write(dir.path().join("photo.jpg"), &photo).unwrap();
        fs::write(dir.path().join("backup/photo.jpg"), &photo).unwrap();
        fs::write(dir.path().join("photo-edited.jpg"), &almost).unwrap();

        let tree = scan(dir.path(), &ScanOptions::default(), &Progress::new()).unwrap();
        let groups = find(&tree, tree.root(), 1);
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].ids.len(), 2);
        assert!(groups[0].wasted() >= 200_000);
    }
}
