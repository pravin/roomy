use std::fs;
use std::path::{Path, PathBuf};

use serde::Serialize;

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
}

/// What we found in a folder.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize)]
pub struct Totals {
    pub bytes: u64,
    pub files: u64,
    /// Things we weren't allowed to read.
    pub errors: u64,
}

/// Adds up the sizes of every file in `root`, and in every folder inside it.
pub fn scan(root: &Path) -> Result<Totals, ScanError> {
    let meta = fs::metadata(root).map_err(|source| ScanError::Io {
        path: root.to_path_buf(),
        source,
    })?;
    if !meta.is_dir() {
        return Err(ScanError::NotAFolder(root.to_path_buf()));
    }
    let mut totals = Totals::default();
    walk(root, &mut totals);
    Ok(totals)
}

fn walk(path: &Path, totals: &mut Totals) {
    let items = match fs::read_dir(path) {
        Ok(items) => items,
        Err(_) => {
            totals.errors += 1;
            return;
        }
    };
    for item in items {
        let Ok(item) = item else {
            totals.errors += 1;
            continue;
        };
        // This doesn't follow symbolic links, which is what we want.
        let Ok(meta) = item.metadata() else {
            totals.errors += 1;
            continue;
        };
        if meta.is_dir() {
            walk(&item.path(), totals);
        } else {
            totals.files += 1;
            totals.bytes += meta.len();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn adds_up_every_file() {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir(dir.path().join("a")).unwrap();
        fs::write(dir.path().join("one"), vec![0u8; 1000]).unwrap();
        fs::write(dir.path().join("a/two"), vec![0u8; 2000]).unwrap();

        let totals = scan(dir.path()).unwrap();
        assert_eq!(
            totals,
            Totals {
                bytes: 3000,
                files: 2,
                errors: 0
            }
        );
    }

    #[test]
    fn a_file_is_not_a_folder() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("file");
        fs::write(&path, "hi").unwrap();
        assert!(matches!(scan(&path), Err(ScanError::NotAFolder(_))));
    }
}
