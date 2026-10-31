// Checks GitHub for a newer Roomy when the app starts, and offers to
// install it. The download is signed, and the updater checks the signature
// against the public key in tauri.conf.json before it installs anything.

import { message } from "@tauri-apps/plugin-dialog";
import { relaunch } from "@tauri-apps/plugin-process";
import { check } from "@tauri-apps/plugin-updater";
import { $, h } from "./dom";

// `asked` is true when someone chose Check for Updates from the menu, so
// they deserve an answer even when there's nothing new.
export async function checkForUpdate(asked = false) {
  let update;
  try {
    update = await check();
  } catch (error) {
    // Offline, or no release yet. Try again next time.
    if (asked) message(`Couldn't check for updates.\n\n${error}`, { title: "Roomy", kind: "warning" });
    return;
  }
  if (!update) {
    if (asked) message("You have the latest version of Roomy.", { title: "Roomy" });
    return;
  }

  const banner = $("#update");
  const button = h("button", {}, "Update and restart");
  button.onclick = async () => {
    button.disabled = true;
    button.textContent = "Downloading…";
    await update.downloadAndInstall();
    await relaunch();
  };
  banner.replaceChildren(h("span", {}, `Roomy ${update.version} is out.`), button);
  banner.hidden = false;
}
