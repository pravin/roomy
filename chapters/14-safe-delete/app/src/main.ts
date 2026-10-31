// Roomy's window: which screen is showing, and the buttons that move
// between them. Each screen has a file of its own.

import { message, open } from "@tauri-apps/plugin-dialog";
import * as api from "./api";
import { formatSeconds, goUp, setScan, setScanner, showFolder } from "./browse";
import { $, formatCount, formatSize } from "./dom";
import { fillIcons } from "./icons";

type Screen = "start" | "scanning" | "results";

let lastPath: string | null = null;

function showScreen(screen: Screen) {
  for (const s of ["start", "scanning", "results"]) {
    $(`#${s}`).hidden = s !== screen;
  }
}

async function scan(path: string) {
  lastPath = path;
  $("#scan-bytes").textContent = "0 B";
  $("#scan-files").textContent = "Starting…";
  $("#scan-time").textContent = "0.0 s";
  $("#scan-current").textContent = path;
  showScreen("scanning");

  const stop = await api.onProgress((p) => {
    $("#scan-bytes").textContent = formatSize(p.bytes);
    $("#scan-files").textContent = `${formatCount(p.files)} files`;
    $("#scan-time").textContent = formatSeconds(p.seconds);
    $("#scan-current").textContent = p.current;
  });
  try {
    setScan(await api.scan(path));
    showScreen("results");
    await showFolder();
  } catch (error) {
    // A cancelled scan just goes back to the start.
    if (!String(error).includes("cancelled")) alertError(error);
    goHome();
  } finally {
    stop();
  }
}

function alertError(error: unknown) {
  message(String(error), { title: "Roomy", kind: "error" });
}

function goHome() {
  showScreen("start");
}

async function chooseFolder(): Promise<string | null> {
  const chosen = await open({ directory: true, title: "Choose a folder to scan" });
  return typeof chosen === "string" ? chosen : null;
}

setScanner(scan);
fillIcons();
$("#choose").onclick = async () => {
  const path = await chooseFolder();
  if (path) scan(path);
};
$("#cancel").onclick = () => api.cancelScan();
$("#up").onclick = () => goUp(true);
$("#rescan").onclick = () => lastPath && scan(lastPath);
$("#new-scan").onclick = () => goHome();

document.addEventListener("keydown", (e) => {
  if ($("#results").hidden) return;
  if (e.key === "Backspace" || (e.key === "ArrowUp" && (e.metaKey || e.altKey))) goUp();
});

goHome();
