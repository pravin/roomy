//! The commands the user interface can call. Each one is a plain Rust
//! function; `#[tauri::command]` does the work of turning JSON from the
//! web page into arguments, and the return value back into JSON.

use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::thread;
use std::time::{Duration, Instant};

use roomy_core::kinds::{self, Category, CategoryTotal};
use roomy_core::treemap::{self, Layout};
use roomy_core::{Kind, NodeId, Progress, Tree, dupes, junk};
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_clipboard_manager::ClipboardExt;
use tauri_plugin_opener::OpenerExt;

use crate::AppState;
use crate::menu::MenuState;
use crate::settings::{self, Settings};

/// A file or folder, as the user interface sees it.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NodeView {
    id: NodeId,
    name: String,
    size: u64,
    files: u64,
    kind: Kind,
    category: Category,
    unreadable: bool,
    has_children: bool,
}

impl NodeView {
    fn new(tree: &Tree, id: NodeId) -> NodeView {
        let node = &tree[id];
        NodeView {
            id,
            name: node.display_name(),
            size: node.size,
            files: node.files,
            kind: node.kind,
            category: Category::of(tree, id),
            unreadable: node.unreadable,
            has_children: !node.children.is_empty(),
        }
    }
}

/// Runs `f` on the current scan, or says there isn't one.
fn with_tree<T>(state: &AppState, f: impl FnOnce(&Tree) -> Result<T, String>) -> Result<T, String> {
    let tree = state.tree.read().unwrap();
    let tree = tree.as_ref().ok_or("nothing has been scanned yet")?;
    f(tree)
}

/// The same, for commands that change the tree.
fn with_tree_mut<T>(
    state: &AppState,
    f: impl FnOnce(&mut Tree) -> Result<T, String>,
) -> Result<T, String> {
    let mut tree = state.tree.write().unwrap();
    let tree = tree.as_mut().ok_or("nothing has been scanned yet")?;
    f(tree)
}

fn check(tree: &Tree, id: NodeId) -> Result<(), String> {
    if tree.contains(id) {
        Ok(())
    } else {
        Err("that isn't there any more".into())
    }
}

#[derive(Serialize)]
pub struct Disk {
    name: String,
    mount: PathBuf,
    total: u64,
    available: u64,
}

#[derive(Serialize)]
pub struct Places {
    home: Option<PathBuf>,
    disks: Vec<Disk>,
    recent: Vec<PathBuf>,
}

/// A folder named on the command line, as in `roomy-app ~/Code`.
#[tauri::command]
pub fn launch_path() -> Option<PathBuf> {
    std::env::args_os()
        .skip(1)
        .map(PathBuf::from)
        .find(|path| path.is_dir())
}

/// Somewhere to start: the home folder, the disks, and recent scans.
#[tauri::command]
pub fn places(app: AppHandle) -> Places {
    // If we can't list the disks, the start screen just doesn't show them.
    let disks = sysinfo::Disks::new_with_refreshed_list()
        .map(|disks| {
            disks
                .iter()
                .filter(|d| {
                    let mount = d.mount_point();
                    d.total_space() > 1_000_000_000
                        && !mount.starts_with("/System/Volumes")
                        && !mount.starts_with("/boot")
                        && !mount.starts_with("/snap")
                })
                .map(|d| Disk {
                    name: d.name().to_string_lossy().into_owned(),
                    mount: d.mount_point().to_path_buf(),
                    total: d.total_space(),
                    available: d.available_space(),
                })
                .collect()
        })
        .unwrap_or_default();
    Places {
        home: app.path().home_dir().ok(),
        disks,
        recent: settings::load(&app).recent,
    }
}

#[derive(Clone, Serialize)]
struct ProgressView {
    files: u64,
    bytes: u64,
    errors: u64,
    current: String,
    seconds: f64,
}

impl ProgressView {
    fn new(progress: &Progress, started: Instant) -> ProgressView {
        ProgressView {
            files: progress.files(),
            bytes: progress.bytes(),
            errors: progress.errors(),
            current: progress.current().display().to_string(),
            seconds: started.elapsed().as_secs_f64(),
        }
    }
}

/// What a finished scan sends back to the window.
#[derive(Serialize)]
pub struct ScanSummary {
    root: NodeView,
    /// How long the scan took.
    seconds: f64,
    /// Things we weren't allowed to read.
    errors: u64,
    /// Folders we didn't look inside, to spare you a popup for each app.
    guarded: u64,
}

/// Scans a folder. While it runs, the window gets a `scan-progress` event
/// ten times a second.
#[tauri::command]
pub async fn scan(app: AppHandle, path: PathBuf) -> Result<ScanSummary, String> {
    let options = settings::load(&app).scan_options();
    let progress = Arc::new(Progress::new());
    *app.state::<AppState>().progress.lock().unwrap() = Some(progress.clone());

    // Scanning takes a while and blocks the thread it's on, so it goes on a
    // thread of its own, away from the ones Tauri uses to answer the window.
    let handle = app.clone();
    let root = path.clone();
    let started = Instant::now();
    let (result, seconds, errors, guarded) = tauri::async_runtime::spawn_blocking(move || {
        thread::scope(|s| {
            let scanner = s.spawn(|| {
                let tree = roomy_core::scan(&root, &options, &progress);
                (tree, started.elapsed().as_secs_f64())
            });
            while !scanner.is_finished() {
                handle
                    .emit("scan-progress", ProgressView::new(&progress, started))
                    .ok();
                thread::sleep(Duration::from_millis(100));
            }
            let (tree, seconds) = scanner.join().unwrap();
            (tree, seconds, progress.errors(), progress.guarded())
        })
    })
    .await
    .map_err(|e| e.to_string())?;

    let state = app.state::<AppState>();
    *state.progress.lock().unwrap() = None;
    let tree = result.map_err(|e| e.to_string())?;
    let summary = ScanSummary {
        root: NodeView::new(&tree, tree.root()),
        seconds,
        errors,
        guarded,
    };
    *state.tree.write().unwrap() = Some(tree);

    // Load the settings again, in case they changed while we were busy.
    let mut settings = settings::load(&app);
    settings.remember(path);
    settings::save(&app, &settings).ok();
    Ok(summary)
}

/// Opens the Full Disk Access page of System Settings, where you can let
/// Roomy see every app's data without being asked about each one.
#[tauri::command]
pub fn full_disk_access() -> Result<(), String> {
    std::process::Command::new("open")
        .arg("x-apple.systempreferences:com.apple.preference.security?Privacy_AllFiles")
        .spawn()
        .map(|_| ())
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn cancel_scan(state: State<AppState>) {
    if let Some(progress) = state.progress.lock().unwrap().as_ref() {
        progress.cancel();
    }
}

#[derive(Serialize)]
pub struct FolderView {
    node: NodeView,
    path: PathBuf,
    /// From the scanned folder down to this one.
    ancestors: Vec<NodeView>,
    children: Vec<NodeView>,
}

/// Everything the window needs to show one folder. Leave out `id` for the
/// folder that was scanned.
#[tauri::command]
pub fn folder(state: State<AppState>, id: Option<NodeId>) -> Result<FolderView, String> {
    with_tree(&state, |tree| {
        let id = id.unwrap_or(tree.root());
        check(tree, id)?;
        Ok(FolderView {
            node: NodeView::new(tree, id),
            path: tree.path(id),
            ancestors: tree
                .ancestors(id)
                .into_iter()
                .map(|a| NodeView::new(tree, a))
                .collect(),
            children: tree[id]
                .children
                .iter()
                .map(|&c| NodeView::new(tree, c))
                .collect(),
        })
    })
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TileView {
    id: NodeId,
    x: f64,
    y: f64,
    w: f64,
    h: f64,
    depth: u32,
    name: String,
    size: u64,
    category: Category,
    has_children: bool,
}

/// Lays out a treemap of a folder for a canvas `width` by `height` pixels.
#[tauri::command]
pub fn treemap(
    state: State<AppState>,
    id: NodeId,
    width: f64,
    height: f64,
) -> Result<Vec<TileView>, String> {
    with_tree(&state, |tree| {
        check(tree, id)?;
        let tiles = treemap::layout(tree, id, width, height, &Layout::default());
        Ok(tiles
            .into_iter()
            .map(|t| {
                let node = &tree[t.id];
                TileView {
                    id: t.id,
                    x: t.rect.x,
                    y: t.rect.y,
                    w: t.rect.w,
                    h: t.rect.h,
                    depth: t.depth,
                    name: node.display_name(),
                    size: node.size,
                    category: Category::of(tree, t.id),
                    has_children: !node.children.is_empty(),
                }
            })
            .collect())
    })
}

#[tauri::command]
pub fn categories(state: State<AppState>, id: NodeId) -> Result<Vec<CategoryTotal>, String> {
    with_tree(&state, |tree| {
        check(tree, id)?;
        Ok(kinds::totals(tree, id))
    })
}

/// Shows a file or folder in Finder, Explorer or the file manager.
#[tauri::command]
pub fn reveal(app: AppHandle, state: State<AppState>, id: NodeId) -> Result<(), String> {
    let path = with_tree(&state, |tree| {
        check(tree, id)?;
        Ok(tree.path(id))
    })?;
    app.opener()
        .reveal_item_in_dir(path)
        .map_err(|e| e.to_string())
}

/// Opens a file with whatever app normally opens it.
#[tauri::command]
pub fn open_item(app: AppHandle, state: State<AppState>, id: NodeId) -> Result<(), String> {
    let path = with_tree(&state, |tree| {
        check(tree, id)?;
        Ok(tree.path(id))
    })?;
    app.opener()
        .open_path(path.to_string_lossy(), None::<&str>)
        .map_err(|e| e.to_string())
}

/// Puts the full path of a file or folder on the clipboard.
#[tauri::command]
pub fn copy_path(app: AppHandle, state: State<AppState>, id: NodeId) -> Result<(), String> {
    let path = with_tree(&state, |tree| {
        check(tree, id)?;
        Ok(tree.path(id))
    })?;
    app.clipboard()
        .write_text(path.to_string_lossy())
        .map_err(|e| e.to_string())
}

#[derive(Serialize)]
pub struct TrashResult {
    freed: u64,
    failed: Vec<String>,
}

#[derive(Clone, Serialize)]
struct TrashProgress {
    done: usize,
    total: usize,
    bytes: u64,
}

/// How many things to hand the Trash at once. On a Mac each batch is one
/// request to the Finder, which takes about the same time for one file as
/// for a hundred, so batching is what makes deleting thousands quick.
const TRASH_BATCH: usize = 100;

/// Moves things to the Trash and takes them out of the tree. While it runs,
/// the window gets a `trash-progress` event after each batch.
#[tauri::command]
pub async fn trash(app: AppHandle, ids: Vec<NodeId>) -> Result<TrashResult, String> {
    app.state::<AppState>()
        .stop_trash
        .store(false, Ordering::Relaxed);
    // Work out what to delete, then let go of the tree while the slow part
    // runs, so the window can carry on drawing.
    let items: Vec<(NodeId, PathBuf, u64)> = with_tree(&app.state::<AppState>(), |tree| {
        let picked: HashSet<NodeId> = ids.iter().copied().collect();
        Ok(ids
            .iter()
            .copied()
            // Skip anything already gone, and anything inside a folder
            // that's going too: it goes to the Trash with its folder.
            .filter(|&id| tree.contains(id) && id != tree.root())
            .filter(|&id| !tree.ancestors(id).iter().any(|a| picked.contains(a)))
            .map(|id| (id, tree.path(id), tree[id].size))
            .collect())
    })?;

    let handle = app.clone();
    let paths: Vec<(PathBuf, u64)> = items.iter().map(|(_, p, s)| (p.clone(), *s)).collect();
    let failed = tauri::async_runtime::spawn_blocking(move || {
        let total = paths.len();
        let (mut done, mut bytes) = (0, 0);
        let mut failed = Vec::new();
        for batch in paths.chunks(TRASH_BATCH) {
            if handle
                .state::<AppState>()
                .stop_trash
                .load(Ordering::Relaxed)
            {
                break;
            }
            let batch_paths: Vec<&PathBuf> = batch.iter().map(|(p, _)| p).collect();
            if trash::delete_all(&batch_paths).is_err() {
                // Something in this batch went wrong. Go through it one at a
                // time to find out what.
                for path in batch_paths {
                    if path.symlink_metadata().is_ok()
                        && let Err(e) = trash::delete(path)
                    {
                        failed.push(format!("{}: {e}", path.display()));
                    }
                }
            }
            done += batch.len();
            bytes += batch.iter().map(|(_, s)| s).sum::<u64>();
            handle
                .emit("trash-progress", TrashProgress { done, total, bytes })
                .ok();
        }
        failed
    })
    .await
    .map_err(|e| e.to_string())?;

    // Take out of the tree whatever really left the disk.
    with_tree_mut(&app.state::<AppState>(), |tree| {
        let mut freed = 0;
        for (id, path, _) in items {
            // A new scan may have replaced the tree while we were busy.
            if tree.contains(id) && tree.path(id) == path && path.symlink_metadata().is_err() {
                freed += tree.remove(id);
            }
        }
        Ok(TrashResult { freed, failed })
    })
}

/// Stops a move to the Trash after the batch it's on. What has gone
/// already stays in the Trash.
#[tauri::command]
pub fn cancel_trash(state: State<AppState>) {
    state.stop_trash.store(true, Ordering::Relaxed);
}

#[derive(Serialize)]
pub struct JunkView {
    id: NodeId,
    name: String,
    description: String,
    size: u64,
    path: PathBuf,
}

/// Developer junk anywhere in the scan, biggest first.
#[tauri::command]
pub fn junk(state: State<AppState>) -> Result<Vec<JunkView>, String> {
    with_tree(&state, |tree| {
        Ok(junk::find(tree, tree.root(), &junk::detectors())
            .into_iter()
            .map(|j| JunkView {
                path: tree.path(j.id),
                id: j.id,
                name: j.name,
                description: j.description,
                size: j.size,
            })
            .collect())
    })
}

#[derive(Serialize)]
pub struct Copy {
    id: NodeId,
    path: PathBuf,
}

#[derive(Serialize)]
pub struct DuplicateView {
    size: u64,
    wasted: u64,
    copies: Vec<Copy>,
}

/// Files that are exact copies of each other. This reads the files, so it
/// runs on its own thread like the scan.
#[tauri::command]
pub async fn duplicates(app: AppHandle) -> Result<Vec<DuplicateView>, String> {
    let min_size = settings::load(&app).min_duplicate_size;
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AppState>();
        with_tree(&state, |tree| {
            Ok(dupes::find(tree, tree.root(), min_size)
                .into_iter()
                .take(500)
                .map(|g| DuplicateView {
                    size: g.size,
                    wasted: g.wasted(),
                    copies: g
                        .ids
                        .iter()
                        .map(|&id| Copy {
                            id,
                            path: tree.path(id),
                        })
                        .collect(),
                })
                .collect())
        })
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub fn get_settings(app: AppHandle) -> Settings {
    settings::load(&app)
}

#[tauri::command]
pub fn save_settings(app: AppHandle, settings: Settings) -> Result<(), String> {
    settings::save(&app, &settings)
}

/// Greys out the menu items that don't make sense right now.
#[tauri::command]
pub fn menu_state(menu: State<MenuState>, scanned: bool, picked: bool) {
    menu.update(scanned, picked);
}
