use std::ffi::OsString;
use std::fs::{self, DirEntry};
use std::path::{Path, PathBuf};

use rayon::prelude::*;

use crate::tree::{Entry, Kind, Tree};

/// The reasons a scan can fail as a whole. A folder we can't read in the
/// middle of a scan isn't one of them: we mark it and carry on.
#[derive(Debug, thiserror::Error)]
pub enum ScanError {
    #[error("can't read {path}: {source}")]
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("{0} is not a folder")]
    NotAFolder(PathBuf),
}

/// Scans `root` and everything below it, using every core you have.
pub fn scan(root: &Path) -> Result<Tree, ScanError> {
    let meta = fs::metadata(root).map_err(|source| ScanError::Io {
        path: root.to_path_buf(),
        source,
    })?;
    if !meta.is_dir() {
        return Err(ScanError::NotAFolder(root.to_path_buf()));
    }
    let entry = scan_dir(root, root.as_os_str().to_owned());
    Ok(Tree::from_entry(root.to_path_buf(), entry))
}

fn scan_dir(path: &Path, name: OsString) -> Entry {
    let mut entry = Entry {
        name,
        kind: Kind::Dir,
        size: 0,
        files: 0,
        unreadable: false,
        children: Vec::new(),
    };
    let items = match fs::read_dir(path) {
        Ok(items) => items,
        Err(_) => {
            entry.unreadable = true;
            return entry;
        }
    };
    // Reading the names in one folder is quick. Looking inside each of the
    // folders it holds is the slow part, so that's what we share out.
    let items: Vec<DirEntry> = items.filter_map(Result::ok).collect();

    entry.children = items.par_iter().filter_map(scan_item).collect();
    entry.size = entry.children.iter().map(|c| c.size).sum();
    entry.files = entry.children.iter().map(|c| c.files).sum();
    entry
}

fn scan_item(item: &DirEntry) -> Option<Entry> {
    // This doesn't follow symbolic links, which is what we want. Following
    // them would count things twice, or go round in circles forever.
    let meta = item.metadata().ok()?;
    let file_type = meta.file_type();

    if file_type.is_dir() {
        return Some(scan_dir(&item.path(), item.file_name()));
    }

    let kind = if file_type.is_symlink() {
        Kind::Symlink
    } else if file_type.is_file() {
        Kind::File
    } else {
        Kind::Other
    };
    Some(Entry {
        name: item.file_name(),
        kind,
        size: meta.len(),
        files: 1,
        unreadable: false,
        children: Vec::new(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn write(path: &Path, bytes: usize) {
        fs::write(path, vec![7u8; bytes]).unwrap();
    }

    #[test]
    fn counts_every_file() {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir(dir.path().join("a")).unwrap();
        fs::create_dir(dir.path().join("a/b")).unwrap();
        write(&dir.path().join("one"), 10_000);
        write(&dir.path().join("a/two"), 20_000);
        write(&dir.path().join("a/b/three"), 30_000);

        let tree = scan(dir.path()).unwrap();
        let root = &tree[tree.root()];
        assert_eq!(root.files, 3);
        assert_eq!(root.size, 60_000);
        // Biggest first: folder `a` holds 50 KB
        assert_eq!(tree[root.children[0]].display_name(), "a");
    }

    #[test]
    fn a_file_is_not_a_folder() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("file");
        write(&path, 1);
        assert!(matches!(scan(&path), Err(ScanError::NotAFolder(_))));
    }

    #[cfg(unix)]
    #[test]
    fn unreadable_folders_are_marked() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let locked = dir.path().join("locked");
        fs::create_dir(&locked).unwrap();
        fs::set_permissions(&locked, fs::Permissions::from_mode(0o000)).unwrap();

        let tree = scan(dir.path());
        fs::set_permissions(&locked, fs::Permissions::from_mode(0o755)).unwrap();

        let tree = tree.unwrap();
        assert!(tree[tree[tree.root()].children[0]].unreadable);
    }
}
