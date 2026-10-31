// Typed wrappers around the Rust commands in src-tauri/src/commands.rs.
// The shapes here mirror the structs that Rust sends back.

import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

export type NodeId = number;

export interface NodeView {
  id: NodeId;
  name: string;
  size: number;
  files: number;
  kind: "file" | "dir" | "symlink" | "other";
  unreadable: boolean;
  hasChildren: boolean;
}

export interface FolderView {
  node: NodeView;
  path: string;
  ancestors: NodeView[];
  children: NodeView[];
}

export interface Progress {
  files: number;
  bytes: number;
  errors: number;
  current: string;
  seconds: number;
}

export interface ScanSummary {
  root: NodeView;
  seconds: number;
  errors: number;
}

export const scan = (path: string) => invoke<ScanSummary>("scan", { path });
export const cancelScan = () => invoke<void>("cancel_scan");
export const folder = (id?: NodeId) => invoke<FolderView>("folder", { id });

export const onProgress = (f: (p: Progress) => void): Promise<UnlistenFn> =>
  listen<Progress>("scan-progress", (e) => f(e.payload));
