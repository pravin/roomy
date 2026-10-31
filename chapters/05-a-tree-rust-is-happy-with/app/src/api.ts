// Typed wrappers around the Rust commands in src-tauri/src/commands.rs.
// The shapes here mirror the structs that Rust sends back.

import { invoke } from "@tauri-apps/api/core";

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
  children: NodeView[];
}

export interface ScanSummary {
  root: NodeView;
}

export const scan = (path: string) => invoke<ScanSummary>("scan", { path });
export const folder = (id?: NodeId) => invoke<FolderView>("folder", { id });
