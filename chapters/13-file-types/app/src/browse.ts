// The Disk tab: the list of what's in a folder, the treemap next to it,
// and the bar along the bottom that says what you've picked.

import * as api from "./api";
import type { NodeId } from "./api";
import { $, formatCount, formatSize, h, percent } from "./dom";
import { icon } from "./icons";
import { COLOURS, NAMES, Treemap } from "./treemap";

// The one thing picked in the list or the treemap.
interface Picked {
  id: NodeId;
  name: string;
  size: number;
  category: api.Category;
  hasChildren: boolean;
}

let current: api.FolderView | null = null;
let picked: Picked | null = null;
let lastScan: api.ScanSummary | null = null;
const map = new Treemap($("#map") as HTMLCanvasElement);
const tooltip = $("#tooltip");

// Remembers how the last scan went, for the bar along the bottom.
export function setScan(summary: api.ScanSummary) {
  lastScan = summary;
}

export function formatSeconds(seconds: number): string {
  return seconds < 1 ? `${Math.ceil(seconds * 1000)} ms` : `${seconds.toFixed(seconds < 10 ? 1 : 0)} s`;
}

export async function showFolder(id?: NodeId) {
  current = await api.folder(id);
  pick(null);
  renderCrumbs();
  renderList();
  renderLegend();
  await map.show(current.node.id);
}

// Shows the same folder again after something in it changed. If the
// folder itself has gone, go back to the top.
export async function refresh() {
  try {
    await showFolder(current?.node.id);
  } catch {
    await showFolder();
  }
}

// Starts a new scan. main.ts hands us its `scan` function, because the
// breadcrumbs and the up button can ask for one.
let scanFolder: (path: string) => void = () => {};
export function setScanner(scan: (path: string) => void) {
  scanFolder = scan;
}

// Goes up one folder. At the top of the scan there's nothing above us in
// the tree, so with `orScan` we scan the enclosing folder instead.
export function goUp(orScan = false) {
  if (!current) return;
  if (current.ancestors.length >= 2) {
    showFolder(current.ancestors[current.ancestors.length - 2].id);
  } else if (orScan) {
    const above = outside(current.ancestors[0].name);
    if (above.length > 0) scanFolder(above[above.length - 1].path);
  }
}

// The folders above the one that was scanned, from the top of the disk
// down. For /Users/me/Code that's /, /Users and /Users/me.
function outside(root: string): { name: string; path: string }[] {
  const windows = root.includes("\\");
  const sep = windows ? "\\" : "/";
  const parts = root.split(sep).filter(Boolean);
  if (parts.length === 0) return []; // The top of the disk
  const chain: { name: string; path: string }[] = [];
  let path = windows ? "" : "/";
  if (!windows) chain.push({ name: "/", path });
  for (const part of parts.slice(0, -1)) {
    path = windows && path === "" ? `${part}${sep}` : path.endsWith(sep) ? path + part : path + sep + part;
    chain.push({ name: part, path });
  }
  return chain;
}

function lastPart(path: string): string {
  return path.split(/[\\/]/).filter(Boolean).pop() ?? path;
}

function renderCrumbs() {
  const crumbs = $("#crumbs");
  const ancestors = current!.ancestors;
  const above = outside(ancestors[0].name);
  const sep = () => h("span", { class: "sep" }, icon("chevron", 12));
  crumbs.replaceChildren();

  // Folders above the scan are dimmed. Clicking one scans it.
  for (const folder of above) {
    crumbs.append(
      h("button", { class: "outside", title: `Scan ${folder.path}`, onclick: () => scanFolder(folder.path) }, folder.name),
      sep(),
    );
  }
  // Folders inside the scan. The last one is where we are, so it isn't a button.
  ancestors.forEach((a, i) => {
    const name = i === 0 ? lastPart(a.name) : a.name;
    if (i > 0) crumbs.append(sep());
    if (i === ancestors.length - 1) {
      crumbs.append(h("span", { class: "here", title: current!.path }, name));
    } else {
      crumbs.append(h("button", { title: a.name, onclick: () => showFolder(a.id) }, name));
    }
  });

  const up = $("#up") as HTMLButtonElement;
  up.disabled = ancestors.length < 2 && above.length === 0;
  up.title = ancestors.length >= 2 ? "Up one folder (Backspace)" : "Scan the enclosing folder";
}

function colourOf(item: { category: api.Category; hasChildren: boolean }): string {
  return item.hasChildren ? "var(--muted)" : COLOURS[item.category];
}

function renderList() {
  const list = $("#list");
  const total = current!.node.size;
  list.replaceChildren(
    ...current!.children.map((child) => {
      const share = total > 0 ? (child.size / total) * 100 : 0;
      return h(
        "div",
        {
          class: `row${child.hasChildren ? " folder" : ""}`,
          "data-id": child.id,
          title: child.unreadable ? "Roomy wasn't allowed to look inside this folder" : child.name,
          onclick: () => pick(child),
          ondblclick: () => open(child),
        },
        h("span", { class: "kind", style: { color: colourOf(child) } }, icon(child.kind === "dir" ? "folder" : "file")),
        h(
          "span",
          { class: "name" },
          child.unreadable ? h("span", { class: "warn" }, icon("warning", 12)) : null,
          child.name,
        ),
        h("span", { class: "size" }, formatSize(child.size)),
        h(
          "div",
          { class: "share" },
          h("div", { class: "bar" }, h("div", { style: { width: `${share}%`, background: colourOf(child) } })),
          h("span", {}, percent(child.size, total)),
        ),
      );
    }),
  );
  if (current!.children.length === 0) {
    list.append(h("p", { class: "empty" }, "This folder is empty."));
  }
}

async function renderLegend() {
  const totals = await api.categories(current!.node.id);
  $("#legend").replaceChildren(
    ...totals.map((t) =>
      h(
        "span",
        {
          class: "chip",
          title: `${formatCount(t.files)} files. Point at this to find them on the map.`,
          onmouseenter: () => map.setHighlight(t.category),
          onmouseleave: () => map.setHighlight(null),
        },
        h("i", { style: { background: COLOURS[t.category] } }),
        NAMES[t.category],
        h("b", {}, formatSize(t.size)),
      ),
    ),
  );
}

function join(folder: string, name: string): string {
  const sep = folder.includes("\\") ? "\\" : "/";
  return folder.endsWith(sep) ? folder + name : folder + sep + name;
}

function open(item: Picked) {
  if (item.hasChildren) showFolder(item.id);
}

function pick(item: Picked | null) {
  picked = item;
  map.selected = item?.id ?? null;
  map.draw();
  for (const row of document.querySelectorAll<HTMLElement>("#list .row")) {
    const on = Number(row.dataset.id) === item?.id;
    row.classList.toggle("selected", on);
    if (on) row.scrollIntoView({ block: "nearest" });
  }
  renderDetails();
}

function renderDetails() {
  const details = $("#details");
  if (!current) return;
  const folder = current.node;
  if (!picked) {
    details.replaceChildren(
      h("span", { class: "kind", style: { color: "var(--muted)" } }, icon("folder", 22)),
      h(
        "div",
        { class: "what" },
        h(
          "div",
          {},
          h("strong", {}, formatSize(folder.size)),
          ` in ${formatCount(folder.files)} files`,
          lastScan ? h("span", { class: "muted" }, ` · scanned in ${formatSeconds(lastScan.seconds)}`) : null,
          lastScan && lastScan.errors > 0
            ? h("span", { class: "muted" }, ` · ${formatCount(lastScan.errors)} couldn't be read`)
            : null,
        ),
        h("div", { class: "path" }, current.path),
      ),
    );
    return;
  }
  const item = picked;
  details.replaceChildren(
    h("span", { class: "kind", style: { color: colourOf(item) } }, icon(item.hasChildren ? "folder" : "file", 22)),
    h(
      "div",
      { class: "what" },
      h(
        "div",
        {},
        h("strong", {}, item.name),
        h("span", { class: "muted" }, ` · ${formatSize(item.size)} · ${percent(item.size, folder.size)} of this folder`),
      ),
      h("div", { class: "path" }, join(current.path, item.name)),
    ),
    ...(item.hasChildren ? [h("button", { onclick: () => open(item) }, icon("open", 15), "Open folder")] : []),
  );
}

// The treemap talks back through these.
map.onSelect = (tile) => pick(tile);
map.onOpen = (tile) => open(tile);
map.onHover = (tile, x, y) => {
  if (!tile) {
    tooltip.hidden = true;
    return;
  }
  const rect = map.canvas.getBoundingClientRect();
  tooltip.replaceChildren(
    h("strong", {}, tile.name),
    h("div", {}, `${formatSize(tile.size)} · ${percent(tile.size, current?.node.size ?? 0)} of this folder`),
    h("small", {}, tile.hasChildren ? "Double-click to open" : NAMES[tile.category]),
  );
  tooltip.hidden = false;
  // Keep the tooltip inside the window.
  const left = Math.min(rect.left + x + 14, window.innerWidth - tooltip.offsetWidth - 8);
  const top = Math.min(rect.top + y + 14, window.innerHeight - tooltip.offsetHeight - 8);
  tooltip.style.left = `${left}px`;
  tooltip.style.top = `${top}px`;
};
