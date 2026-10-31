use std::env;
use std::path::PathBuf;
use std::process::ExitCode;

use roomy_core::format_size;

fn main() -> ExitCode {
    // The folder to look at is the first argument, or the current folder.
    let path = env::args_os()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));

    match roomy_core::scan(&path) {
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
