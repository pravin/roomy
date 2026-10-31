// Things you can do to a file, shared by every part of the window.

import { ask, message } from "@tauri-apps/plugin-dialog";
import * as api from "./api";
import { formatSize } from "./dom";

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

  const result = await api.trash(ids);
  if (result.failed.length > 0) {
    await message(`Some things couldn't be moved:\n\n${result.failed.slice(0, 8).join("\n")}`, {
      title: "Roomy",
      kind: "error",
    });
  }
  return result.freed;
}
