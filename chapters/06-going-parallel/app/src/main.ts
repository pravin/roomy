// Roomy's window: a start screen, a scanning screen, and the results.

import { message, open } from "@tauri-apps/plugin-dialog";
import * as api from "./api";
import { $, formatCount, formatSize, h } from "./dom";
import { fillIcons, icon } from "./icons";

type Screen = "start" | "scanning" | "results";

function showScreen(screen: Screen) {
  for (const s of ["start", "scanning", "results"]) {
    $(`#${s}`).hidden = s !== screen;
  }
}

function formatSeconds(seconds: number): string {
  return seconds < 1 ? `${Math.ceil(seconds * 1000)} ms` : `${seconds.toFixed(seconds < 10 ? 1 : 0)} s`;
}

async function scan(path: string) {
  $("#scan-current").textContent = path;
  showScreen("scanning");
  try {
    const summary = await api.scan(path);
    await showResults(summary);
  } catch (error) {
    message(String(error), { title: "Roomy", kind: "error" });
    showScreen("start");
  }
}

async function showResults(summary: api.ScanSummary) {
  const folder = await api.folder();
  const root = summary.root;
  $("#total").textContent = formatSize(root.size);
  $("#summary").textContent =
    `in ${formatCount(root.files)} files, scanned in ${formatSeconds(summary.seconds)}`;
  $("#folder-name").textContent = folder.path;
  $("#biggest").replaceChildren(
    ...folder.children.slice(0, 15).map((child) =>
      h(
        "div",
        { class: "item" },
        icon(child.kind === "dir" ? "folder" : "file"),
        h("span", { class: "label" }, child.name),
        h("span", { class: "size" }, formatSize(child.size)),
      ),
    ),
  );
  showScreen("results");
}

async function chooseFolder() {
  const chosen = await open({ directory: true, title: "Choose a folder to scan" });
  if (typeof chosen === "string") scan(chosen);
}

fillIcons();
$("#choose").onclick = chooseFolder;
$("#again").onclick = chooseFolder;
