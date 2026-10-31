// The Developer junk tab: build folders and caches you can throw away.

import * as api from "./api";
import { trashItems, trashName } from "./actions";
import { $, formatCount, formatSize, h } from "./dom";
import { icon } from "./icons";
import { refresh } from "./browse";

const panel = $("#tab-junk");

// Finds the junk and puts its total on the tab, so people see it without
// having to go and look.
export async function loadJunk(): Promise<api.JunkItem[]> {
  const items = await api.junk();
  const total = items.reduce((sum, i) => sum + i.size, 0);
  const badge = $("#junk-badge");
  badge.textContent = formatSize(total);
  badge.hidden = items.length === 0;
  return items;
}

// The folder a junk folder belongs to, like "website" for
// /home/me/website/node_modules.
function project(path: string): string {
  const parts = path.split(/[\\/]/).filter(Boolean);
  return parts[parts.length - 2] ?? path;
}

export async function showJunk() {
  panel.replaceChildren(h("p", { class: "empty" }, "Looking for junk…"));
  const items = await loadJunk();
  if (items.length === 0) {
    panel.replaceChildren(
      h(
        "div",
        { class: "empty big-empty" },
        icon("sparkle", 40),
        h("h2", {}, "No developer junk"),
        h("p", {}, "No node_modules, no build folders, no caches. Tidy!"),
      ),
    );
    return;
  }

  // Everything starts ticked. People came here to clean up.
  const ticked = new Set(items.map((i) => i.id));
  const boxes = new Map<api.NodeId, HTMLInputElement>();
  const selected = h("span", { class: "total" });
  const clean = h("button", { class: "danger" });

  const update = () => {
    const size = items.filter((i) => ticked.has(i.id)).reduce((sum, i) => sum + i.size, 0);
    selected.replaceChildren(h("b", {}, formatSize(size)), ` selected in ${formatCount(ticked.size)} folders`);
    clean.replaceChildren(icon("trash", 15), `Move to ${trashName}`);
    clean.disabled = ticked.size === 0;
  };
  const tick = (id: api.NodeId, on: boolean) => {
    if (on) ticked.add(id);
    else ticked.delete(id);
    boxes.get(id)!.checked = on;
  };

  clean.onclick = async () => {
    const chosen = items.filter((i) => ticked.has(i.id));
    const size = chosen.reduce((sum, i) => sum + i.size, 0);
    const what = chosen.length === 1 ? `“${chosen[0].name}”` : `${chosen.length} folders`;
    const freed = await trashItems(chosen.map((i) => i.id), what, size);
    if (freed === null) return;
    await refresh();
    await showJunk();
  };

  // Totals for each kind of junk. Clicking one ticks or unticks them all.
  const kinds = new Map<string, { size: number; ids: api.NodeId[] }>();
  for (const item of items) {
    const kind = kinds.get(item.name) ?? { size: 0, ids: [] };
    kind.size += item.size;
    kind.ids.push(item.id);
    kinds.set(item.name, kind);
  }
  const total = items.reduce((sum, i) => sum + i.size, 0);

  panel.replaceChildren(
    h(
      "header",
      { class: "panel-head" },
      h("div", {}, h("h2", {}, "Developer junk"), h("p", { class: "lead" },
        "Folders that programming tools make and can make again: installed packages, build output and download caches. Deleting them is safe, but the next build will take longer while they come back.")),
      h("div", { class: "headline" }, h("b", {}, formatSize(total)), h("span", {}, `in ${formatCount(items.length)} folders`)),
    ),
    h(
      "div",
      { class: "kinds" },
      ...[...kinds].map(([name, kind]) =>
        h(
          "button",
          {
            class: "chip",
            title: "Tick or untick all of these",
            onclick: () => {
              const on = !kind.ids.every((id) => ticked.has(id));
              kind.ids.forEach((id) => tick(id, on));
              update();
            },
          },
          name,
          h("b", {}, formatSize(kind.size)),
        ),
      ),
    ),
    h(
      "div",
      { class: "items" },
      ...items.map((item) => {
        const box = h("input", {
          type: "checkbox",
          checked: true,
          onchange: (e: Event) => {
            tick(item.id, (e.target as HTMLInputElement).checked);
            update();
          },
        });
        boxes.set(item.id, box);
        return h(
          "label",
          { class: "item", title: item.description },
          box,
          h("span", { class: "label" }, h("span", { class: "badge" }, item.name), h("b", {}, project(item.path))),
          h("span", { class: "size" }, formatSize(item.size)),
          h("small", { class: "path" }, item.path),
        );
      }),
    ),
    h("div", { class: "action-bar" }, selected, clean),
  );
  update();
}
