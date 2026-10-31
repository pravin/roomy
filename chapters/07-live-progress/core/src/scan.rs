use std::ffi::OsString;
use std::fs::{self, DirEntry};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use rayon::prelude::*;

use crate::tree::{Entry, Kind, Tree};

/// The reasons a scan can fail as a whole. A folder we can't read in the
/// middle of a scan isn't one of them: we count it and carry on.
#[derive(Debug, thiserror::Error)]
pub enum ScanError {
    #[error("can't read {path}: {source}")]
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("{0} is not a folder")]
    NotAFolder(PathBuf),
    #[error("the scan was cancelled")]
    Cancelled,
}

/// Counters the scanner updates as it goes. Any thread can read them while
/// a scan is running, which is how the app shows a live total.
#[derive(Debug, Default)]
pub struct Progress {
    files: AtomicU64,
    bytes: AtomicU64,
    errors: AtomicU64,
    cancelled: AtomicBool,
    current: Mutex<PathBuf>,
}

impl Progress {
    pub fn new() -> Progress {
        Progress::default()
    }

    pub fn files(&self) -> u64 {
        self.files.load(Ordering::Relaxed)
    }

    pub fn bytes(&self) -> u64 {
        self.bytes.load(Ordering::Relaxed)
    }

    pub fn errors(&self) -> u64 {
        self.errors.load(Ordering::Relaxed)
    }

    /// The folder the scanner was in most recently.
    pub fn current(&self) -> PathBuf {
        self.current.lock().map(|p| p.clone()).unwrap_or_default()
    }

    /// Asks a running scan to stop as soon as it can.
    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::Relaxed);
    }

    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Relaxed)
    }

    fn error(&self) {
        self.errors.fetch_add(1, Ordering::Relaxed);
    }
}

/// Scans `root` and everything below it, using every core you have.
pub fn scan(root: &Path, progress: &Progress) -> Result<Tree, ScanError> {
    let meta = fs::metadata(root).map_err(|source| ScanError::Io {
        path: root.to_path_buf(),
        source,
    })?;
    if !meta.is_dir() {
        return Err(ScanError::NotAFolder(root.to_path_buf()));
    }

    let entry = scan_dir(progress, root, root.as_os_str().to_owned());

    if progress.is_cancelled() {
        return Err(ScanError::Cancelled);
    }
    Ok(Tree::from_entry(root.to_path_buf(), entry))
}

fn scan_dir(progress: &Progress, path: &Path, name: OsString) -> Entry {
    let mut entry = Entry {
        name,
        kind: Kind::Dir,
        size: 0,
        files: 0,
        unreadable: false,
        children: Vec::new(),
    };
    if progress.is_cancelled() {
        return entry;
    }
    if let Ok(mut current) = progress.current.try_lock() {
        path.clone_into(&mut current);
    }

    let items = match fs::read_dir(path) {
        Ok(items) => items,
        Err(_) => {
            progress.error();
            entry.unreadable = true;
            return entry;
        }
    };
    // Reading the names in one folder is quick. Looking inside each of the
    // folders it holds is the slow part, so that's what we share out.
    let items: Vec<DirEntry> = items
        .filter_map(|item| item.inspect_err(|_| progress.error()).ok())
        .collect();

    entry.children = items
        .par_iter()
        .filter_map(|item| scan_item(progress, item))
        .collect();
    entry.size = entry.children.iter().map(|c| c.size).sum();
    entry.files = entry.children.iter().map(|c| c.files).sum();
    entry
}

fn scan_item(progress: &Progress, item: &DirEntry) -> Option<Entry> {
    let path = item.path();
    // This doesn't follow symbolic links, which is what we want. Following
    // them would count things twice, or go round in circles forever.
    let meta = match item.metadata() {
        Ok(meta) => meta,
        Err(_) => {
            progress.error();
            return None;
        }
    };
    let file_type = meta.file_type();

    if file_type.is_dir() {
        return Some(scan_dir(progress, &path, item.file_name()));
    }

    let kind = if file_type.is_symlink() {
        Kind::Symlink
    } else if file_type.is_file() {
        Kind::File
    } else {
        Kind::Other
    };
    let size = meta.len();
    progress.files.fetch_add(1, Ordering::Relaxed);
    progress.bytes.fetch_add(size, Ordering::Relaxed);
    Some(Entry {
        name: item.file_name(),
        kind,
        size,
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

        let progress = Progress::new();
        let tree = scan(dir.path(), &progress).unwrap();
        let root = &tree[tree.root()];
        assert_eq!(root.files, 3);
        assert_eq!(progress.files(), 3);
        assert!(root.size >= 60_000);
        // Biggest first: folder `a` holds 50 KB
        assert_eq!(tree[root.children[0]].display_name(), "a");
    }

    #[test]
    fn a_file_is_not_a_folder() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("file");
        write(&path, 1);
        let result = scan(&path, &Progress::new());
        assert!(matches!(result, Err(ScanError::NotAFolder(_))));
    }

    #[cfg(unix)]
    #[test]
    fn unreadable_folders_are_counted() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let locked = dir.path().join("locked");
        fs::create_dir(&locked).unwrap();
        fs::set_permissions(&locked, fs::Permissions::from_mode(0o000)).unwrap();

        let progress = Progress::new();
        let tree = scan(dir.path(), &progress);
        fs::set_permissions(&locked, fs::Permissions::from_mode(0o755)).unwrap();

        let tree = tree.unwrap();
        assert_eq!(progress.errors(), 1);
        assert!(tree[tree[tree.root()].children[0]].unreadable);
    }

    #[test]
    fn cancelling_stops_the_scan() {
        let dir = tempfile::tempdir().unwrap();
        let progress = Progress::new();
        progress.cancel();
        let result = scan(dir.path(), &progress);
        assert!(matches!(result, Err(ScanError::Cancelled)));
    }
}
