//! The menu bar. On macOS it sits at the top of the screen, and on Windows
//! and Linux at the top of the window. Clicking an item sends its id to the
//! web page as a `menu` event, and the page decides what to do.

use tauri::menu::{AboutMetadata, Menu, MenuItem, PredefinedMenuItem, Submenu, SubmenuBuilder};
use tauri::{AppHandle, Emitter, Wry};
use tauri_plugin_opener::OpenerExt;

const REPO: &str = "https://github.com/pravin/roomy";

/// The items that only make sense some of the time, kept so we can grey
/// them out. Tauri can't look items up inside submenus, so we hold on to
/// them ourselves.
pub struct MenuState {
    /// Need a finished scan.
    scanned: Vec<MenuItem<Wry>>,
    /// Need something picked in the list or the treemap.
    picked: Vec<MenuItem<Wry>>,
}

impl MenuState {
    pub fn update(&self, scanned: bool, picked: bool) {
        for item in &self.scanned {
            item.set_enabled(scanned).ok();
        }
        for item in &self.picked {
            item.set_enabled(picked).ok();
        }
    }
}

/// What the file manager is called here, for "Show in Finder".
fn file_manager() -> &'static str {
    if cfg!(target_os = "macos") {
        "Finder"
    } else if cfg!(windows) {
        "Explorer"
    } else {
        "Files"
    }
}

pub fn build(app: &AppHandle) -> tauri::Result<(Menu<Wry>, MenuState)> {
    // A small helper, so each item is one line below. `None` means no
    // keyboard shortcut. "CmdOrCtrl" is ⌘ on a Mac and Ctrl elsewhere.
    let item = |id: &str, text: &str, enabled: bool, keys: Option<&str>| {
        MenuItem::with_id(app, id, text, enabled, keys)
    };
    let mac = cfg!(target_os = "macos");

    let about = PredefinedMenuItem::about(
        app,
        Some("About Roomy"),
        Some(AboutMetadata {
            name: Some("Roomy".into()),
            version: Some(app.package_info().version.to_string()),
            authors: Some(vec!["Pravin Paratey".into()]),
            comments: Some("See what's filling your disk.".into()),
            copyright: Some("© 2026 Pravin Paratey".into()),
            license: Some("Apache-2.0".into()),
            website: Some(REPO.into()),
            website_label: Some("github.com/pravin/roomy".into()),
            ..Default::default()
        }),
    )?;
    let settings = item("settings", "Settings…", true, Some("CmdOrCtrl+,"))?;

    let rescan = item("rescan", "Scan Again", false, Some("CmdOrCtrl+R"))?;
    let mut file = SubmenuBuilder::new(app, "File")
        .item(&item(
            "scan-folder",
            "Scan Folder…",
            true,
            Some("CmdOrCtrl+O"),
        )?)
        .item(&item(
            "scan-home",
            "Scan Home Folder",
            true,
            Some("CmdOrCtrl+Shift+H"),
        )?)
        .item(&rescan)
        .separator()
        .item(&item(
            "start",
            "Start Screen",
            true,
            Some("CmdOrCtrl+Shift+N"),
        )?)
        .separator()
        .close_window();
    if !mac {
        file = file
            .separator()
            .item(&settings)
            .separator()
            .quit_with_text("Exit");
    }

    let view_disk = item("view-disk", "Disk", false, Some("CmdOrCtrl+1"))?;
    let view_junk = item("view-junk", "Developer Junk", false, Some("CmdOrCtrl+2"))?;
    let view_dupes = item("view-dupes", "Duplicates", false, Some("CmdOrCtrl+3"))?;
    let go_up = item("go-up", "Enclosing Folder", false, Some("CmdOrCtrl+Up"))?;
    let go_top = item("go-top", "Top Folder", false, Some("CmdOrCtrl+Shift+Up"))?;
    let view = SubmenuBuilder::new(app, "View")
        .item(&view_disk)
        .item(&view_junk)
        .item(&view_dupes)
        .separator()
        .item(&go_up)
        .item(&go_top)
        .build()?;

    let open = item("open", "Open", false, Some("CmdOrCtrl+Down"))?;
    let reveal = item(
        "reveal",
        &format!("Show in {}", file_manager()),
        false,
        Some("CmdOrCtrl+Alt+R"),
    )?;
    let copy_path = item("copy-path", "Copy Path", false, Some("CmdOrCtrl+Alt+C"))?;
    let trash = item(
        "trash",
        if cfg!(windows) {
            "Move to Recycle Bin"
        } else {
            "Move to Trash"
        },
        false,
        Some("CmdOrCtrl+Backspace"),
    )?;
    let item_menu = SubmenuBuilder::new(app, "Item")
        .item(&open)
        .item(&reveal)
        .item(&copy_path)
        .separator()
        .item(&trash)
        .build()?;

    let mut help = SubmenuBuilder::new(app, "Help")
        .item(&item("help-repo", "Roomy on GitHub", true, None)?)
        .item(&item("help-tutorial", "Read the Tutorial", true, None)?)
        .item(&item("help-issue", "Report a Problem…", true, None)?);
    if !mac {
        help = help.separator().item(&about);
    }

    let mut menus: Vec<Submenu<Wry>> = Vec::new();
    if mac {
        // On a Mac the first menu is always named after the app.
        menus.push(
            SubmenuBuilder::new(app, "Roomy")
                .item(&about)
                .separator()
                .item(&settings)
                .separator()
                .services()
                .separator()
                .hide_with_text("Hide Roomy")
                .hide_others()
                .show_all()
                .separator()
                .quit_with_text("Quit Roomy")
                .build()?,
        );
    }
    menus.extend([file.build()?, view, item_menu]);
    if mac {
        menus.push(
            SubmenuBuilder::new(app, "Window")
                .minimize()
                .maximize()
                .build()?,
        );
    }
    menus.push(help.build()?);

    let menu = Menu::new(app)?;
    for submenu in &menus {
        menu.append(submenu)?;
    }
    let state = MenuState {
        scanned: vec![rescan, view_disk, view_junk, view_dupes, go_up, go_top],
        picked: vec![open, reveal, copy_path, trash],
    };
    Ok((menu, state))
}

/// Runs when any menu item is clicked. Links open in the browser here;
/// everything else is the web page's job.
pub fn clicked(app: &AppHandle, id: &str) {
    let link = match id {
        "help-repo" => Some(REPO.to_string()),
        "help-tutorial" => Some(format!("{REPO}#the-chapters")),
        "help-issue" => Some(format!("{REPO}/issues/new")),
        _ => None,
    };
    match link {
        Some(url) => {
            app.opener().open_url(url, None::<&str>).ok();
        }
        None => {
            app.emit("menu", id).ok();
        }
    }
}
