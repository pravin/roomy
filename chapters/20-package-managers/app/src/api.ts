// Typed wrappers around the Rust commands in src-tauri/src/commands.rs.
// The shapes here mirror the structs that Rust sends back.

import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

export type NodeId = number;

export type Category =
  | "folder" | "video" | "image" | "audio" | "archive"
  | "document" | "code" | "app" | "other";

export interface NodeView {
  id: NodeId;
  name: string;
  size: number;
  files: number;
  kind: "file" | "dir" | "symlink" | "other";
  category: Category;
  unreadable: boolean;
  hasChildren: boolean;
}

export interface FolderView {
  node: NodeView;
  path: string;
  ancestors: NodeView[];
  children: NodeView[];
}

export interface Tile {
  id: NodeId;
  x: number;
  y: number;
  w: number;
  h: number;
  depth: number;
  name: string;
  size: number;
  category: Category;
  hasChildren: boolean;
}

export interface CategoryTotal {
  category: Category;
  size: number;
  files: number;
}

export interface Progress {
  files: number;
  bytes: number;
  errors: number;
  current: string;
  seconds: number;
}

export interface TrashProgress {
  done: number;
  total: number;
  bytes: number;
}

export interface ScanSummary {
  root: NodeView;
  seconds: number;
  errors: number;
  guarded: number;
}

export interface Disk {
  name: string;
  mount: string;
  total: number;
  available: number;
}

export interface Places {
  home: string | null;
  disks: Disk[];
  recent: string[];
}

export interface JunkItem {
  id: NodeId;
  name: string;
  description: string;
  size: number;
  path: string;
}

export interface DuplicateGroup {
  size: number;
  wasted: number;
  copies: { id: NodeId; path: string }[];
}

export interface TrashResult {
  freed: number;
  failed: string[];
}

export interface Settings {
  oneFileSystem: boolean;
  exclude: string[];
  minDuplicateSize: number;
  recent: string[];
}

export const places = () => invoke<Places>("places");
export const launchPath = () => invoke<string | null>("launch_path");
export const scan = (path: string) => invoke<ScanSummary>("scan", { path });
export const cancelScan = () => invoke<void>("cancel_scan");
export const cancelTrash = () => invoke<void>("cancel_trash");
export const fullDiskAccess = () => invoke<void>("full_disk_access");
export const folder = (id?: NodeId) => invoke<FolderView>("folder", { id });
export const treemap = (id: NodeId, width: number, height: number) =>
  invoke<Tile[]>("treemap", { id, width, height });
export const categories = (id: NodeId) => invoke<CategoryTotal[]>("categories", { id });
export const reveal = (id: NodeId) => invoke<void>("reveal", { id });
export const openItem = (id: NodeId) => invoke<void>("open_item", { id });
export const copyPath = (id: NodeId) => invoke<void>("copy_path", { id });
export const trash = (ids: NodeId[]) => invoke<TrashResult>("trash", { ids });
export const junk = () => invoke<JunkItem[]>("junk");
export const duplicates = () => invoke<DuplicateGroup[]>("duplicates");
export const getSettings = () => invoke<Settings>("get_settings");
export const saveSettings = (settings: Settings) => invoke<void>("save_settings", { settings });
export const menuState = (scanned: boolean, picked: boolean) =>
  invoke<void>("menu_state", { scanned, picked });

export const onProgress = (f: (p: Progress) => void): Promise<UnlistenFn> =>
  listen<Progress>("scan-progress", (e) => f(e.payload));

export const onTrashProgress = (f: (p: TrashProgress) => void): Promise<UnlistenFn> =>
  listen<TrashProgress>("trash-progress", (e) => f(e.payload));
