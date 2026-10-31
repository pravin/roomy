use std::env;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use roomy_core::{Tree, format_size};

fn main() -> ExitCode {
    // The folder to look at is the first argument, or the current folder.
    let path = env::args_os()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));
    match top(&path, 20) {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("roomy: {message}");
            ExitCode::FAILURE
        }
    }
}

/// Scans a folder and says what it found.
fn scan(path: &Path) -> Result<Tree, String> {
    let tree = roomy_core::scan(path).map_err(|e| e.to_string())?;
    let root = &tree[tree.root()];
    eprintln!(
        "Scanned {}: {} files, {}",
        path.display(),
        thousands(root.files),
        format_size(root.size)
    );
    Ok(tree)
}

fn top(path: &Path, count: usize) -> Result<(), String> {
    let tree = scan(path)?;
    let root = &tree[tree.root()];
    println!();
    for &id in root.children.iter().take(count) {
        let node = &tree[id];
        let slash = if node.children.is_empty() { "" } else { "/" };
        println!(
            "{:>8}  {}  {}{slash}",
            format_size(node.size),
            bar(node.size, root.size, 20),
            node.display_name()
        );
    }
    if root.children.len() > count {
        println!("     ...  and {} more", root.children.len() - count);
    }
    Ok(())
}

/// A bar like `██████░░░░` showing `part` out of `whole`.
fn bar(part: u64, whole: u64, width: usize) -> String {
    let filled = if whole == 0 {
        0
    } else {
        ((part as f64 / whole as f64) * width as f64).round() as usize
    };
    format!("{}{}", "█".repeat(filled), "░".repeat(width - filled))
}

/// 1234567 becomes "1,234,567".
fn thousands(n: u64) -> String {
    let digits = n.to_string();
    let mut out = String::new();
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn thousands_get_commas() {
        assert_eq!(thousands(7), "7");
        assert_eq!(thousands(1234567), "1,234,567");
    }

    #[test]
    fn bars_fill_up() {
        assert_eq!(bar(5, 10, 4), "██░░");
        assert_eq!(bar(0, 0, 2), "░░");
    }
}
