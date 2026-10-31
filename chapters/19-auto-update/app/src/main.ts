// Roomy's window: which screen is showing, and the buttons that move
// between them. Each screen has a file of its own.

import { getVersion } from "@tauri-apps/api/app";
import { message } from "@tauri-apps/plugin-dialog";
import { openUrl } from "@tauri-apps/plugin-opener";
import * as api from "./api";
import { actOnPicked, formatSeconds, goTop, goUp, hasPicked, redrawMap, setScan, setScanner, showFolder } from "./browse";
import { $, formatCount, formatSize } from "./dom";
import { showDuplicates } from "./dupes";
import { fillIcons } from "./icons";
import { loadJunk, showJunk } from "./junk";
import { onMenu, setPicked, setScanned } from "./menu";
import { chooseFolder, editSettings, showPlaces } from "./start";
import { checkForUpdate } from "./update";

type Screen = "start" | "scanning" | "results";
type Tab = "map" | "junk" | "dupes";

let lastPath: string | null = null;

function showScreen(screen: Screen) {
  for (const s of ["start", "scanning", "results"]) {
    $(`#${s}`).hidden = s !== screen;
  }
  setScanned(screen === "results");
}

function showTab(tab: Tab) {
  for (const button of document.querySelectorAll<HTMLButtonElement>(".tabs button")) {
    button.classList.toggle("active", button.dataset.tab === tab);
  }
  for (const t of ["map", "junk", "dupes"]) {
    $(`#tab-${t}`).hidden = t !== tab;
  }
  $("#details").hidden = tab !== "map";
  // The Item menu only works on the Disk tab.
  setPicked(tab === "map" && hasPicked());
  if (tab === "map") redrawMap();
  if (tab === "junk") showJunk();
  if (tab === "dupes") showDuplicates();
}

// `expected` is how many bytes we think we'll find, when we know.
async function scan(path: string, expected?: number) {
  lastPath = path;
  const bar = $("#scan-progress");
  const fill = bar.firstElementChild as HTMLElement;
  bar.hidden = !expected;
  fill.style.width = "0%";
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
    if (expected) fill.style.width = `${Math.min((p.bytes / expected) * 100, 100)}%`;
  });
  try {
    setScan(await api.scan(path));
    showScreen("results");
    showTab("map");
    await showFolder();
    loadJunk();
  } catch (error) {
    // A cancelled scan just goes back to the start.
    if (!String(error).includes("cancelled")) alertError(error);
    await goHome();
  } finally {
    stop();
  }
}

function alertError(error: unknown) {
  message(String(error), { title: "Roomy", kind: "error" });
}

async function goHome() {
  showScreen("start");
  await showPlaces(scan);
}

setScanner(scan);
fillIcons();
$("#choose").onclick = async () => {
  const path = await chooseFolder();
  if (path) scan(path);
};
$("#open-settings").onclick = () => editSettings();
$("#cancel").onclick = () => api.cancelScan();
$("#up").onclick = () => goUp(true);
$("#rescan").onclick = () => lastPath && scan(lastPath);
$("#new-scan").onclick = () => goHome();
for (const button of document.querySelectorAll<HTMLButtonElement>(".tabs button")) {
  button.onclick = () => showTab(button.dataset.tab as Tab);
}
// Links open in the web browser, not inside Roomy's window.
$("#repo").onclick = (e) => {
  e.preventDefault();
  openUrl((e.currentTarget as HTMLAnchorElement).href);
};
getVersion().then((v) => ($("#version").textContent = `Version ${v}`));

// Backspace goes up a folder. With ⌘ or Ctrl it's a menu shortcut instead.
document.addEventListener("keydown", (e) => {
  if ($("#results").hidden || $("#tab-map").hidden || e.metaKey || e.ctrlKey) return;
  if (e.key === "Backspace") goUp();
});

async function scanHome() {
  const { home } = await api.places();
  if (home) scan(home);
}

onMenu((id) => {
  const results = !$("#results").hidden;
  switch (id) {
    case "scan-folder":
      chooseFolder().then((path) => {
        if (path) scan(path);
      });
      break;
    case "scan-home":
      scanHome();
      break;
    case "rescan":
      if (lastPath) scan(lastPath);
      break;
    case "start":
      goHome();
      break;
    case "settings":
      editSettings();
      break;
    case "check-updates":
      checkForUpdate(true);
      break;
    case "view-disk":
    case "view-junk":
    case "view-dupes":
      if (results) showTab(({ "view-disk": "map", "view-junk": "junk", "view-dupes": "dupes" } as const)[id]);
      break;
    case "go-up":
      if (results) goUp(true);
      break;
    case "go-top":
      if (results) goTop();
      break;
    default:
      // open, reveal, copy-path and trash act on what's picked.
      if (results && !$("#tab-map").hidden) actOnPicked(id);
  }
});

// The default right-click menu has Reload in it, which would lose the scan.
document.addEventListener("contextmenu", (e) => e.preventDefault());

// `roomy-app ~/Code` goes straight to scanning that folder.
api.launchPath().then((path) => (path ? scan(path) : goHome()));
checkForUpdate();
