use std::env;
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

fn main() -> ExitCode {
    let path = env::args_os()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));

    match scan(&path) {
        Ok(totals) => {
            println!(
                "{}: {} in {} files",
                path.display(),
                format_size(totals.bytes),
                totals.files
            );
            if totals.errors > 0 {
                println!("({} things couldn't be read)", totals.errors);
            }
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("roomy: {error}");
            ExitCode::FAILURE
        }
    }
}

/// The reasons a scan can fail as a whole. A folder we can't read in the
/// middle of a scan isn't one of them: we count it and carry on.
#[derive(Debug)]
enum ScanError {
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
    NotAFolder(PathBuf),
}

// How a ScanError looks when it's printed with {}.
impl fmt::Display for ScanError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            ScanError::Io { path, source } => write!(f, "can't read {}: {source}", path.display()),
            ScanError::NotAFolder(path) => write!(f, "{} is not a folder", path.display()),
        }
    }
}

impl std::error::Error for ScanError {}

/// What we found in a folder.
#[derive(Debug, Default, PartialEq)]
struct Totals {
    bytes: u64,
    files: u64,
    /// Things we weren't allowed to read.
    errors: u64,
}

/// Adds up the sizes of every file in `root`, and in every folder inside it.
fn scan(root: &Path) -> Result<Totals, ScanError> {
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

/// Turns a number of bytes into something a person can read, like "4.2 GB".
///
/// We count in thousands, the way macOS, Ubuntu and disk manufacturers do,
/// so the numbers Roomy shows match the ones on the box.
fn format_size(bytes: u64) -> String {
    const UNITS: [&str; 6] = ["KB", "MB", "GB", "TB", "PB", "EB"];

    if bytes < 1000 {
        return format!("{bytes} B");
    }
    let mut value = bytes as f64;
    let mut unit = "B";
    for u in UNITS {
        value /= 1000.0;
        unit = u;
        if value < 1000.0 {
            break;
        }
    }
    if value < 10.0 {
        format!("{value:.1} {unit}")
    } else {
        format!("{value:.0} {unit}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn small_numbers_are_bytes() {
        assert_eq!(format_size(0), "0 B");
        assert_eq!(format_size(999), "999 B");
    }

    #[test]
    fn big_numbers_get_units() {
        assert_eq!(format_size(1_000), "1.0 KB");
        assert_eq!(format_size(1_536_000), "1.5 MB");
        assert_eq!(format_size(42_000_000_000), "42 GB");
        assert_eq!(format_size(u64::MAX), "18 EB");
    }

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

    #[cfg(unix)]
    #[test]
    fn locked_folders_are_counted_not_fatal() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let locked = dir.path().join("locked");
        fs::create_dir(&locked).unwrap();
        fs::set_permissions(&locked, fs::Permissions::from_mode(0o000)).unwrap();
        let totals = scan(dir.path());
        fs::set_permissions(&locked, fs::Permissions::from_mode(0o755)).unwrap();
        assert_eq!(totals.unwrap().errors, 1);
    }
}
