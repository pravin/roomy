// Roomy's window: a start screen, a scanning screen, and the results.

import { message, open } from "@tauri-apps/plugin-dialog";
import * as api from "./api";
import { $, formatCount, formatSize } from "./dom";
import { fillIcons } from "./icons";

type Screen = "start" | "scanning" | "results";

function showScreen(screen: Screen) {
  for (const s of ["start", "scanning", "results"]) {
    $(`#${s}`).hidden = s !== screen;
  }
}

async function scan(path: string) {
  $("#scan-current").textContent = path;
  showScreen("scanning");
  try {
    const totals = await api.scan(path);
    $("#total").textContent = formatSize(totals.bytes);
    $("#summary").textContent =
      `in ${formatCount(totals.files)} files in ${path}` +
      (totals.errors > 0 ? `. ${formatCount(totals.errors)} things couldn't be read.` : "");
    showScreen("results");
  } catch (error) {
    message(String(error), { title: "Roomy", kind: "error" });
    showScreen("start");
  }
}

async function chooseFolder() {
  const chosen = await open({ directory: true, title: "Choose a folder to scan" });
  if (typeof chosen === "string") scan(chosen);
}

fillIcons();
$("#choose").onclick = chooseFolder;
$("#again").onclick = chooseFolder;
