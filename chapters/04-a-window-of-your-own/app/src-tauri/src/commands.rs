//! The commands the user interface can call. Each one is a plain Rust
//! function; `#[tauri::command]` does the work of turning JSON from the
//! web page into arguments, and the return value back into JSON.

use std::path::PathBuf;

use roomy_core::Totals;

/// Adds up everything in a folder.
#[tauri::command]
pub async fn scan(path: PathBuf) -> Result<Totals, String> {
    // Scanning takes a while and blocks the thread it's on, so it goes on a
    // thread of its own, away from the ones Tauri uses to answer the window.
    tauri::async_runtime::spawn_blocking(move || roomy_core::scan(&path))
        .await
        .map_err(|e| e.to_string())?
        .map_err(|e| e.to_string())
}
