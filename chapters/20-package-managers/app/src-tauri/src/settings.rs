use std::fs;
use std::path::PathBuf;

use roomy_core::ScanOptions;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};

/// Everything the user can change, saved as JSON between runs.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Settings {
    pub one_file_system: bool,
    pub exclude: Vec<PathBuf>,
    pub min_duplicate_size: u64,
    /// Folders scanned lately, newest first.
    pub recent: Vec<PathBuf>,
}

impl Default for Settings {
    fn default() -> Settings {
        let options = ScanOptions::recommended();
        Settings {
            one_file_system: options.one_file_system,
            exclude: options.exclude,
            min_duplicate_size: 1_000_000,
            recent: Vec::new(),
        }
    }
}

impl Settings {
    pub fn scan_options(&self) -> ScanOptions {
        ScanOptions {
            one_file_system: self.one_file_system,
            exclude: self.exclude.clone(),
            // Not saved: it depends on whether you've given Roomy Full
            // Disk Access, which can change between one scan and the next.
            guarded: roomy_core::scan::guarded_folders(),
        }
    }

    pub fn remember(&mut self, path: PathBuf) {
        self.recent.retain(|p| *p != path);
        self.recent.insert(0, path);
        self.recent.truncate(5);
    }
}

fn file(app: &AppHandle) -> Option<PathBuf> {
    app.path()
        .app_config_dir()
        .ok()
        .map(|dir| dir.join("settings.json"))
}

/// Reads the settings file. A missing or broken file gives the defaults,
/// because nobody wants an app that won't start over a settings file.
pub fn load(app: &AppHandle) -> Settings {
    file(app)
        .and_then(|path| fs::read_to_string(path).ok())
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default()
}

pub fn save(app: &AppHandle, settings: &Settings) -> Result<(), String> {
    let path = file(app).ok_or("there's nowhere to save settings")?;
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    let text = serde_json::to_string_pretty(settings).map_err(|e| e.to_string())?;
    fs::write(path, text).map_err(|e| e.to_string())
}
