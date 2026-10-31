// The Duplicates tab: files that are exact copies of each other.

import * as api from "./api";
import { trashItems, trashName } from "./actions";
import { $, formatCount, formatSize, h } from "./dom";
import { icon } from "./icons";
import { refresh } from "./browse";

const panel = $("#tab-dupes");

export async function showDuplicates() {
  panel.replaceChildren(
    h(
      "div",
      { class: "empty big-empty" },
      h("div", { class: "spinner" }),
      h("p", {}, "Comparing files. This reads them, so it can take a minute…"),
    ),
  );
  const groups = await api.duplicates();
  if (groups.length === 0) {
    panel.replaceChildren(
      h(
        "div",
        { class: "empty big-empty" },
        icon("copies", 40),
        h("h2", {}, "No duplicates"),
        h("p", {}, "Every file here is one of a kind."),
      ),
    );
    return;
  }

  // Tick every copy but the first, so one of each is always kept.
  const ticked = new Set(groups.flatMap((g) => g.copies.slice(1).map((c) => c.id)));
  const sizeOf = new Map(groups.flatMap((g) => g.copies.map((c) => [c.id, g.size] as const)));
  const selected = h("span", { class: "total" });
  const clean = h("button", { class: "danger" });
  const wasted = groups.reduce((sum, g) => sum + g.wasted, 0);
  const selectedSize = () => [...ticked].reduce((sum, id) => sum + (sizeOf.get(id) ?? 0), 0);

  const update = () => {
    selected.replaceChildren(h("b", {}, formatSize(selectedSize())), ` selected in ${formatCount(ticked.size)} files`);
    clean.replaceChildren(icon("trash", 15), `Move to ${trashName}`);
    clean.disabled = ticked.size === 0;
  };

  clean.onclick = async () => {
    // Never let every copy in a group go.
    for (const g of groups) {
      if (g.copies.every((c) => ticked.has(c.id))) {
        ticked.delete(g.copies[0].id);
      }
    }
    const freed = await trashItems([...ticked], `${ticked.size} duplicate files`, selectedSize());
    if (freed === null) return;
    await refresh();
    await showDuplicates();
  };

  panel.replaceChildren(
    h(
      "header",
      { class: "panel-head" },
      h("div", {}, h("h2", {}, "Duplicates"), h("p", { class: "lead" },
        "Files that are byte for byte the same. We've ticked every copy but the first, so you keep one of each.")),
      h("div", { class: "headline" }, h("b", {}, formatSize(wasted)), h("span", {}, "could be saved")),
    ),
    ...groups.map((g) =>
      h(
        "section",
        { class: "group" },
        h("h3", {}, `${g.copies.length} copies of ${formatSize(g.size)}`),
        h(
          "div",
          { class: "items" },
          ...g.copies.map((copy) =>
            h(
              "label",
              { class: "item" },
              h("input", {
                type: "checkbox",
                checked: ticked.has(copy.id),
                onchange: (e: Event) => {
                  if ((e.target as HTMLInputElement).checked) ticked.add(copy.id);
                  else ticked.delete(copy.id);
                  update();
                },
              }),
              h("span", { class: "label path", title: copy.path }, copy.path),
              h(
                "button",
                {
                  class: "ghost icon-only",
                  title: "Show",
                  onclick: (e: Event) => {
                    e.preventDefault();
                    api.reveal(copy.id);
                  },
                },
                icon("reveal", 15),
              ),
            ),
          ),
        ),
      ),
    ),
    h("div", { class: "action-bar" }, selected, clean),
  );
  update();
}
