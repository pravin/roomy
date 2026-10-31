//! The commands the user interface can call. Each one is a plain Rust
//! function; `#[tauri::command]` does the work of turning JSON from the
//! web page into arguments, and the return value back into JSON.

use std::path::PathBuf;
use std::time::Instant;

use roomy_core::{Kind, NodeId, Tree};
use serde::Serialize;
use tauri::{AppHandle, Manager, State};

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

/// What a finished scan sends back to the window.
#[derive(Serialize)]
pub struct ScanSummary {
    root: NodeView,
    /// How long the scan took.
    seconds: f64,
}

/// Scans a folder and keeps the result for the other commands to use.
#[tauri::command]
pub async fn scan(app: AppHandle, path: PathBuf) -> Result<ScanSummary, String> {
    // Scanning takes a while and blocks the thread it's on, so it goes on a
    // thread of its own, away from the ones Tauri uses to answer the window.
    let (result, seconds) = tauri::async_runtime::spawn_blocking(move || {
        let started = Instant::now();
        let tree = roomy_core::scan(&path);
        (tree, started.elapsed().as_secs_f64())
    })
    .await
    .map_err(|e| e.to_string())?;

    let tree = result.map_err(|e| e.to_string())?;
    let summary = ScanSummary {
        root: NodeView::new(&tree, tree.root()),
        seconds,
    };
    *app.state::<AppState>().tree.write().unwrap() = Some(tree);
    Ok(summary)
}

#[derive(Serialize)]
pub struct FolderView {
    node: NodeView,
    path: PathBuf,
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
            children: tree[id]
                .children
                .iter()
                .map(|&c| NodeView::new(tree, c))
                .collect(),
        })
    })
}
