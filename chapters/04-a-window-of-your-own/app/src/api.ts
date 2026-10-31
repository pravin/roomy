// Typed wrappers around the Rust commands in src-tauri/src/commands.rs.
// The shapes here mirror the structs that Rust sends back.

import { invoke } from "@tauri-apps/api/core";

export interface Totals {
  bytes: number;
  files: number;
  errors: number;
}

export const scan = (path: string) => invoke<Totals>("scan", { path });
