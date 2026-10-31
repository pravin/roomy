// The start screen and the settings dialog.

import { open } from "@tauri-apps/plugin-dialog";
import * as api from "./api";
import { $, formatSize, h } from "./dom";
import { icon } from "./icons";

// `scan` gets the path, and for a whole disk, how much we expect to find,
// so the scanning screen can show a proper progress bar.
export async function showPlaces(scan: (path: string, expected?: number) => void) {
  const places = await api.places();
  const cards: HTMLElement[] = [];

  if (places.home) {
    cards.push(
      h(
        "button",
        { class: "place", onclick: () => scan(places.home!) },
        h("span", { class: "place-icon" }, icon("home", 22)),
        h("span", { class: "place-text" }, h("strong", {}, "Home folder"), h("span", {}, places.home)),
        h("span", { class: "go" }, icon("chevron", 16)),
      ),
    );
  }
  for (const disk of places.disks) {
    const used = disk.total - disk.available;
    const share = (used / disk.total) * 100;
    cards.push(
      h(
        "button",
        { class: "place", onclick: () => scan(disk.mount, used) },
        h("span", { class: "place-icon" }, icon("disk", 22)),
        h(
          "span",
          { class: "place-text" },
          h("strong", {}, disk.name || disk.mount),
          h("span", {}, `${formatSize(disk.available)} free of ${formatSize(disk.total)}`),
          h("span", { class: `meter${share > 90 ? " full" : ""}` }, h("span", { style: { width: `${share}%` } })),
        ),
        h("span", { class: "go" }, icon("chevron", 16)),
      ),
    );
  }
  $("#places").replaceChildren(...cards);

  $("#recent").replaceChildren(
    ...(places.recent.length > 0 ? [h("span", {}, "Recent")] : []),
    ...places.recent.map((path) =>
      h("button", { class: "chip", title: path, onclick: () => scan(path) }, icon("folder", 14), lastPart(path)),
    ),
  );
}

function lastPart(path: string): string {
  return path.split(/[\\/]/).filter(Boolean).pop() ?? path;
}

export async function chooseFolder(): Promise<string | null> {
  const chosen = await open({ directory: true, title: "Choose a folder to scan" });
  return typeof chosen === "string" ? chosen : null;
}

export async function editSettings() {
  const settings = await api.getSettings();
  const dialog = $("#settings") as HTMLDialogElement;
  const oneFs = $("#set-one-fs") as HTMLInputElement;
  const minDupe = $("#set-min-dupe") as HTMLSelectElement;
  const exclude = [...settings.exclude];

  const renderExclude = () => {
    $("#set-exclude").replaceChildren(
      ...exclude.map((path, i) =>
        h(
          "li",
          {},
          path,
          h(
            "button",
            { type: "button", class: "icon-only", title: "Remove", onclick: () => (exclude.splice(i, 1), renderExclude()) },
            icon("close", 14),
          ),
        ),
      ),
    );
  };

  oneFs.checked = settings.oneFileSystem;
  minDupe.value = String(settings.minDuplicateSize);
  renderExclude();
  $("#set-add-exclude").onclick = async () => {
    const path = await chooseFolder();
    if (path && !exclude.includes(path)) {
      exclude.push(path);
      renderExclude();
    }
  };

  dialog.onclose = async () => {
    if (dialog.returnValue !== "save") return;
    await api.saveSettings({
      ...settings,
      oneFileSystem: oneFs.checked,
      minDuplicateSize: Number(minDupe.value),
      exclude,
    });
  };
  dialog.showModal();
}
