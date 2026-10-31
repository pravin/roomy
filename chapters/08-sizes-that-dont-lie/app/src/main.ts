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
  $("#scan-bytes").textContent = "0 B";
  $("#scan-files").textContent = "Starting…";
  $("#scan-time").textContent = "0.0 s";
  $("#scan-current").textContent = path;
  showScreen("scanning");

  // Rust tells us how it's getting on ten times a second.
  const stop = await api.onProgress((p) => {
    $("#scan-bytes").textContent = formatSize(p.bytes);
    $("#scan-files").textContent = `${formatCount(p.files)} files`;
    $("#scan-time").textContent = formatSeconds(p.seconds);
    $("#scan-current").textContent = p.current;
  });
  try {
    const summary = await api.scan(path);
    await showResults(summary);
  } catch (error) {
    // A cancelled scan just goes back to the start.
    if (!String(error).includes("cancelled")) {
      message(String(error), { title: "Roomy", kind: "error" });
    }
    showScreen("start");
  } finally {
    stop();
  }
}

async function showResults(summary: api.ScanSummary) {
  const folder = await api.folder();
  const root = summary.root;
  $("#total").textContent = formatSize(root.size);
  $("#summary").textContent =
    `in ${formatCount(root.files)} files, scanned in ${formatSeconds(summary.seconds)}` +
    (summary.errors > 0 ? `. ${formatCount(summary.errors)} couldn't be read.` : "");
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
$("#cancel").onclick = () => api.cancelScan();
