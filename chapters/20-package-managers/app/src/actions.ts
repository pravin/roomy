// Things you can do to a file, shared by every part of the window.

import { ask, message } from "@tauri-apps/plugin-dialog";
import * as api from "./api";
import { $, formatCount, formatSize } from "./dom";

const platform = navigator.userAgent;

// What the file manager is called here.
export const fileManager = platform.includes("Mac")
  ? "Finder"
  : platform.includes("Windows")
    ? "Explorer"
    : "Files";

export const trashName = platform.includes("Windows") ? "Recycle Bin" : "Trash";

// Asks first, then moves things to the Trash. Returns the bytes freed, or
// null if the user changed their mind.
export async function trashItems(ids: api.NodeId[], what: string, size: number): Promise<number | null> {
  const sure = await ask(
    `Move ${what} (${formatSize(size)}) to the ${trashName}?\n\nYou can put things back from the ${trashName} until you empty it.`,
    { title: "Roomy", kind: "warning", okLabel: `Move to ${trashName}`, cancelLabel: "Cancel" },
  );
  if (!sure) return null;

  // Moving thousands of files takes a while, so cover the window with a
  // progress bar. It also stops you clicking on things that are going.
  const overlay = $("#trashing");
  const fill = overlay.querySelector<HTMLElement>(".progress > div")!;
  $("#trash-title").textContent = `Moving ${what} to the ${trashName}`;
  $("#trash-stats").textContent = `0 of ${formatSize(size)}`;
  fill.style.width = "0%";
  const stop = $("#trash-cancel") as HTMLButtonElement;
  stop.disabled = false;
  stop.textContent = "Stop";
  stop.onclick = () => {
    stop.disabled = true;
    stop.textContent = "Stopping…";
    api.cancelTrash();
  };
  overlay.hidden = false;
  const unlisten = await api.onTrashProgress((p) => {
    fill.style.width = `${(p.done / p.total) * 100}%`;
    $("#trash-stats").textContent =
      p.total > 1
        ? `${formatSize(p.bytes)} of ${formatSize(size)} · ${formatCount(p.done)} of ${formatCount(p.total)}`
        : `${formatSize(p.bytes)} of ${formatSize(size)}`;
  });
  let result: api.TrashResult;
  try {
    result = await api.trash(ids);
  } finally {
    unlisten();
    overlay.hidden = true;
  }
  if (result.failed.length > 0) {
    await message(`Some things couldn't be moved:\n\n${result.failed.slice(0, 8).join("\n")}`, {
      title: "Roomy",
      kind: "error",
    });
  }
  return result.freed;
}
