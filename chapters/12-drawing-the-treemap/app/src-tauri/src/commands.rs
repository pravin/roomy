//! The commands the user interface can call. Each one is a plain Rust
//! function; `#[tauri::command]` does the work of turning JSON from the
//! web page into arguments, and the return value back into JSON.

use std::path::PathBuf;
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use roomy_core::treemap::{self, Layout};
use roomy_core::{Kind, NodeId, Progress, ScanOptions, Tree};
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State};

use crate::AppState;

/// A file or folder, as the user interface sees it.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NodeView {
    id: NodeId,
    name: String,
    size: u64,
    files: u64,
    kind: Kind,
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

fn check(tree: &Tree, id: NodeId) -> Result<(), String> {
    match tree.get(id) {
        Some(_) => Ok(()),
        None => Err("there's no such file".into()),
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
}

/// Scans a folder. While it runs, the window gets a `scan-progress` event
/// ten times a second.
#[tauri::command]
pub async fn scan(app: AppHandle, path: PathBuf) -> Result<ScanSummary, String> {
    let options = ScanOptions::recommended();
    let progress = Arc::new(Progress::new());
    *app.state::<AppState>().progress.lock().unwrap() = Some(progress.clone());

    // Scanning takes a while and blocks the thread it's on, so it goes on a
    // thread of its own, away from the ones Tauri uses to answer the window.
    let handle = app.clone();
    let root = path;
    let started = Instant::now();
    let (result, seconds, errors) = tauri::async_runtime::spawn_blocking(move || {
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
            (tree, seconds, progress.errors())
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
    };
    *state.tree.write().unwrap() = Some(tree);
    Ok(summary)
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
                    has_children: !node.children.is_empty(),
                }
            })
            .collect())
    })
}
