// Don't open a console window next to the app on Windows, in release builds.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod commands;

use std::sync::{Arc, Mutex, RwLock};

use roomy_core::{Progress, Tree};

/// What the app remembers between one command and the next.
#[derive(Default)]
pub struct AppState {
    /// The last finished scan. Lots of commands can read it at once, but
    /// only one at a time can change it.
    tree: RwLock<Option<Tree>>,
    /// The scan in progress, so it can be cancelled.
    progress: Mutex<Option<Arc<Progress>>>,
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(AppState::default())
        .invoke_handler(tauri::generate_handler![
            commands::scan,
            commands::cancel_scan,
            commands::folder,
            commands::treemap,
            commands::categories,
        ])
        .run(tauri::generate_context!())
        .expect("Roomy couldn't start");
}
