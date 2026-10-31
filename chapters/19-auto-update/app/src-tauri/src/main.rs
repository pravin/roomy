// Don't open a console window next to the app on Windows, in release builds.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod commands;
mod menu;
mod settings;

use std::sync::{Arc, Mutex, RwLock};

use roomy_core::{Progress, Tree};
use tauri::Manager;

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
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .manage(AppState::default())
        .setup(|app| {
            let (menu, state) = menu::build(app.handle())?;
            app.set_menu(menu)?;
            app.manage(state);
            Ok(())
        })
        .on_menu_event(|app, event| menu::clicked(app, event.id().as_ref()))
        .invoke_handler(tauri::generate_handler![
            commands::launch_path,
            commands::places,
            commands::scan,
            commands::cancel_scan,
            commands::folder,
            commands::treemap,
            commands::categories,
            commands::reveal,
            commands::open_item,
            commands::copy_path,
            commands::trash,
            commands::junk,
            commands::duplicates,
            commands::get_settings,
            commands::save_settings,
            commands::menu_state,
        ])
        .run(tauri::generate_context!())
        .expect("Roomy couldn't start");
}
