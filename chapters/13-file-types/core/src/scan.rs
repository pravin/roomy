use std::collections::HashSet;
use std::ffi::OsString;
use std::fs::{self, DirEntry, Metadata};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use rayon::prelude::*;
use serde::{Deserialize, Serialize};

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

/// Choices that change what gets scanned.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct ScanOptions {
    /// Stay on the drive we started on. Don't wander into other disks,
    /// network shares or anything else mounted inside it.
    pub one_file_system: bool,
    /// Folders to leave out completely.
    pub exclude: Vec<PathBuf>,
}

impl ScanOptions {
    /// The options Roomy uses unless you tell it otherwise.
    pub fn recommended() -> ScanOptions {
        ScanOptions {
            // On macOS your home folder lives on a different volume from
            // `/`, so staying on one file system would skip it. We use
            // the exclude list there instead.
            one_file_system: !cfg!(target_os = "macos"),
            exclude: platform::default_excludes(),
        }
    }
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

/// Everything the scanning threads share.
struct Context<'a> {
    options: &'a ScanOptions,
    progress: &'a Progress,
    root_device: u64,
    /// Hard links we have already counted, as (device, inode).
    seen_links: Mutex<HashSet<(u64, u64)>>,
}

/// Scans `root` and everything below it, using every core you have.
pub fn scan(root: &Path, options: &ScanOptions, progress: &Progress) -> Result<Tree, ScanError> {
    let meta = fs::metadata(root).map_err(|source| ScanError::Io {
        path: root.to_path_buf(),
        source,
    })?;
    if !meta.is_dir() {
        return Err(ScanError::NotAFolder(root.to_path_buf()));
    }

    let ctx = Context {
        options,
        progress,
        root_device: platform::device(&meta),
        seen_links: Mutex::new(HashSet::new()),
    };
    let mut entry = scan_dir(&ctx, root, root.as_os_str().to_owned());
    entry.size += platform::disk_size(&meta);

    if progress.is_cancelled() {
        return Err(ScanError::Cancelled);
    }
    Ok(Tree::from_entry(root.to_path_buf(), entry))
}

fn scan_dir(ctx: &Context, path: &Path, name: OsString) -> Entry {
    let mut entry = Entry {
        name,
        kind: Kind::Dir,
        size: 0,
        files: 0,
        unreadable: false,
        children: Vec::new(),
    };
    if ctx.progress.is_cancelled() {
        return entry;
    }
    if let Ok(mut current) = ctx.progress.current.try_lock() {
        path.clone_into(&mut current);
    }

    let items = match fs::read_dir(path) {
        Ok(items) => items,
        Err(_) => {
            ctx.progress.error();
            entry.unreadable = true;
            return entry;
        }
    };
    // Reading the names in one folder is quick. Looking inside each of the
    // folders it holds is the slow part, so that's what we share out.
    let items: Vec<DirEntry> = items
        .filter_map(|item| item.inspect_err(|_| ctx.progress.error()).ok())
        .collect();

    entry.children = items
        .par_iter()
        .filter_map(|item| scan_item(ctx, item))
        .collect();
    entry.size = entry.children.iter().map(|c| c.size).sum();
    entry.files = entry.children.iter().map(|c| c.files).sum();
    entry
}

fn scan_item(ctx: &Context, item: &DirEntry) -> Option<Entry> {
    let path = item.path();
    if ctx.options.exclude.contains(&path) {
        return None;
    }
    // This doesn't follow symbolic links, which is what we want. Following
    // them would count things twice, or go round in circles forever.
    let meta = match item.metadata() {
        Ok(meta) => meta,
        Err(_) => {
            ctx.progress.error();
            return None;
        }
    };
    let file_type = meta.file_type();

    if file_type.is_dir() {
        if ctx.options.one_file_system && platform::device(&meta) != ctx.root_device {
            return None;
        }
        let mut entry = scan_dir(ctx, &path, item.file_name());
        entry.size += platform::disk_size(&meta);
        return Some(entry);
    }

    let kind = if file_type.is_symlink() {
        Kind::Symlink
    } else if file_type.is_file() {
        Kind::File
    } else {
        Kind::Other
    };
    let size = if first_sighting(ctx, &meta) {
        platform::disk_size(&meta)
    } else {
        0
    };
    ctx.progress.files.fetch_add(1, Ordering::Relaxed);
    ctx.progress.bytes.fetch_add(size, Ordering::Relaxed);
    Some(Entry {
        name: item.file_name(),
        kind,
        size,
        files: 1,
        unreadable: false,
        children: Vec::new(),
    })
}

/// A file with hard links shows up in more than one folder, but its bytes
/// are only on the disk once. We count it the first time we see it.
fn first_sighting(ctx: &Context, meta: &Metadata) -> bool {
    match platform::link_key(meta) {
        Some(key) => ctx.seen_links.lock().unwrap().insert(key),
        None => true,
    }
}

#[cfg(unix)]
mod platform {
    use std::fs::Metadata;
    use std::os::unix::fs::MetadataExt;
    use std::path::PathBuf;

    /// Space actually used on disk. Unix counts it in 512 byte blocks, no
    /// matter how big the file system's real blocks are.
    pub fn disk_size(meta: &Metadata) -> u64 {
        meta.blocks() * 512
    }

    pub fn device(meta: &Metadata) -> u64 {
        meta.dev()
    }

    pub fn link_key(meta: &Metadata) -> Option<(u64, u64)> {
        (meta.nlink() > 1).then(|| (meta.dev(), meta.ino()))
    }

    pub fn default_excludes() -> Vec<PathBuf> {
        let paths: &[&str] = if cfg!(target_os = "macos") {
            // /System/Volumes/Data is your data again, seen from another angle.
            &["/System/Volumes", "/dev"]
        } else {
            // Files the kernel makes up on the spot. They take no space.
            &["/proc", "/sys", "/dev", "/run"]
        };
        paths.iter().map(PathBuf::from).collect()
    }
}

#[cfg(not(unix))]
mod platform {
    use std::fs::Metadata;
    use std::path::PathBuf;

    /// Windows does have a way to ask for the space used on disk
    /// (`GetCompressedFileSizeW`), but the standard library doesn't call it.
    /// The file's length is close enough, except for compressed files.
    pub fn disk_size(meta: &Metadata) -> u64 {
        meta.len()
    }

    /// Each Windows drive has its own letter, so we don't get to another
    /// drive by accident. Pretend everything is on the same one.
    pub fn device(_meta: &Metadata) -> u64 {
        0
    }

    pub fn link_key(_meta: &Metadata) -> Option<(u64, u64)> {
        None
    }

    pub fn default_excludes() -> Vec<PathBuf> {
        Vec::new()
    }
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
        let tree = scan(dir.path(), &ScanOptions::default(), &progress).unwrap();
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
        let result = scan(&path, &ScanOptions::default(), &Progress::new());
        assert!(matches!(result, Err(ScanError::NotAFolder(_))));
    }

    #[test]
    fn excluded_folders_are_skipped() {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir(dir.path().join("skip")).unwrap();
        write(&dir.path().join("skip/big"), 100_000);
        let options = ScanOptions {
            exclude: vec![dir.path().join("skip")],
            ..ScanOptions::default()
        };
        let tree = scan(dir.path(), &options, &Progress::new()).unwrap();
        assert_eq!(tree[tree.root()].files, 0);
    }

    #[cfg(unix)]
    #[test]
    fn hard_links_count_once() {
        let dir = tempfile::tempdir().unwrap();
        write(&dir.path().join("original"), 100_000);
        fs::hard_link(dir.path().join("original"), dir.path().join("link")).unwrap();
        let tree = scan(dir.path(), &ScanOptions::default(), &Progress::new()).unwrap();
        let root = &tree[tree.root()];
        assert_eq!(root.files, 2);
        assert!(root.size < 200_000);
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
        let tree = scan(dir.path(), &ScanOptions::default(), &progress);
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
        let result = scan(dir.path(), &ScanOptions::default(), &progress);
        assert!(matches!(result, Err(ScanError::Cancelled)));
    }
}
