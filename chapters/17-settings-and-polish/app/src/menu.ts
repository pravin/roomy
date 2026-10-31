// Keeps the menu bar in step with the window: greys out what doesn't make
// sense yet, and hands clicks on to whoever wants them.

import { listen } from "@tauri-apps/api/event";
import * as api from "./api";

const state = { scanned: false, picked: false };

export function setScanned(scanned: boolean) {
  state.scanned = scanned;
  if (!scanned) state.picked = false;
  api.menuState(state.scanned, state.picked);
}

export function setPicked(picked: boolean) {
  state.picked = picked;
  api.menuState(state.scanned, state.picked);
}

// `handler` gets the id of the item that was clicked, like "scan-folder".
export function onMenu(handler: (id: string) => void) {
  listen<string>("menu", (e) => handler(e.payload));
}
