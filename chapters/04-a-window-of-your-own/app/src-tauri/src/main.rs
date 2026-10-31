// Don't open a console window next to the app on Windows, in release builds.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod commands;

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![commands::scan])
        .run(tauri::generate_context!())
        .expect("Roomy couldn't start");
}
