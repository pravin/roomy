use std::io::{self, BufRead, IsTerminal, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::thread;
use std::time::{Duration, Instant};

use clap::{Parser, Subcommand};
use roomy_core::{Progress, ScanOptions, Tree, format_size, junk};

/// Find out what's filling your disk.
#[derive(Parser)]
#[command(version, about, args_conflicts_with_subcommands = true)]
struct Cli {
    /// The folder to look at
    #[arg(default_value = ".")]
    path: PathBuf,

    /// How many of the biggest things to show
    #[arg(short = 'n', long, default_value_t = 20)]
    top: usize,

    #[command(flatten)]
    scan: ScanArgs,

    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(clap::Args)]
struct ScanArgs {
    /// Follow into other drives and mounted file systems
    #[arg(long, global = true)]
    cross_mounts: bool,
}

#[derive(Subcommand)]
enum Command {
    /// List developer junk (node_modules, target, caches...) you can delete
    Junk {
        #[arg(default_value = ".")]
        path: PathBuf,
        /// Move everything found to the Trash
        #[arg(long)]
        clean: bool,
        /// Don't ask before cleaning
        #[arg(short, long)]
        yes: bool,
    },
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    let result = match cli.command {
        None => top(&cli.path, cli.top, &cli.scan),
        Some(Command::Junk {
            ref path,
            clean,
            yes,
        }) => junk(path, clean, yes, &cli.scan),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("roomy: {message}");
            ExitCode::FAILURE
        }
    }
}

/// Scans a folder, showing a running total on the terminal while it works.
fn scan(path: &Path, args: &ScanArgs) -> Result<Tree, String> {
    let mut options = ScanOptions::recommended();
    if args.cross_mounts {
        options.one_file_system = false;
    }
    let progress = Progress::new();
    let started = Instant::now();
    let show = io::stderr().is_terminal();

    // A scoped thread can borrow `progress`, because Rust knows the thread
    // ends before this function does.
    let tree = thread::scope(|s| {
        let scanner = s.spawn(|| roomy_core::scan(path, &options, &progress));
        while !scanner.is_finished() {
            if show {
                eprint!(
                    "\rScanning... {} files, {}   ",
                    thousands(progress.files()),
                    format_size(progress.bytes())
                );
            }
            thread::sleep(Duration::from_millis(100));
        }
        scanner.join().unwrap()
    });
    if show {
        eprint!("\r{:60}\r", "");
    }
    let tree = tree.map_err(|e| e.to_string())?;

    let root = &tree[tree.root()];
    eprintln!(
        "Scanned {} in {:.1}s: {} files, {}",
        path.display(),
        started.elapsed().as_secs_f64(),
        thousands(root.files),
        format_size(root.size)
    );
    if progress.errors() > 0 {
        eprintln!("({} things couldn't be read)", thousands(progress.errors()));
    }
    Ok(tree)
}

fn top(path: &Path, count: usize, args: &ScanArgs) -> Result<(), String> {
    let tree = scan(path, args)?;
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

fn junk(path: &Path, clean: bool, yes: bool, args: &ScanArgs) -> Result<(), String> {
    let tree = scan(path, args)?;
    let found = junk::find(&tree, tree.root(), &junk::detectors());
    if found.is_empty() {
        println!("No developer junk here. Tidy!");
        return Ok(());
    }

    let total: u64 = found.iter().map(|j| j.size).sum();
    println!();
    for item in &found {
        println!(
            "{:>8}  {}",
            format_size(item.size),
            tree.path(item.id).display()
        );
    }
    println!("\n{} in {} folders", format_size(total), found.len());

    if !clean {
        println!("Run again with --clean to move them to the Trash.");
        return Ok(());
    }
    if !yes && !confirm(&format!("Move {} to the Trash?", format_size(total))) {
        return Ok(());
    }
    let paths: Vec<PathBuf> = found.iter().map(|j| tree.path(j.id)).collect();
    trash::delete_all(&paths).map_err(|e| e.to_string())?;
    println!("Done. {} moved to the Trash.", format_size(total));
    Ok(())
}

fn confirm(question: &str) -> bool {
    print!("{question} [y/N] ");
    io::stdout().flush().ok();
    let mut answer = String::new();
    io::stdin().lock().read_line(&mut answer).ok();
    matches!(answer.trim(), "y" | "Y" | "yes")
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
