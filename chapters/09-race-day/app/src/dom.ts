// A few small helpers so we don't need a framework.

type Child = Node | string | null | undefined | false;

// Makes an element. `h("p", { class: "note" }, "Hello")` is <p class="note">Hello</p>.
// Text always goes in as text, never as HTML, so a file called
// "<script>.txt" can't do anything nasty.
export function h<K extends keyof HTMLElementTagNameMap>(
  tag: K,
  props: Record<string, unknown> = {},
  ...children: Child[]
): HTMLElementTagNameMap[K] {
  const el = document.createElement(tag);
  for (const [key, value] of Object.entries(props)) {
    if (value === undefined || value === null || value === false) continue;
    if (key.startsWith("on") && typeof value === "function") {
      el.addEventListener(key.slice(2), value as EventListener);
    } else if (key === "class") {
      el.className = String(value);
    } else if (key === "style" && typeof value === "object") {
      Object.assign(el.style, value);
    } else if (value === true) {
      el.setAttribute(key, "");
    } else {
      el.setAttribute(key, String(value));
    }
  }
  for (const child of children) {
    if (child === null || child === undefined || child === false) continue;
    el.append(child);
  }
  return el;
}

export function $(selector: string): HTMLElement {
  const el = document.querySelector<HTMLElement>(selector);
  if (!el) throw new Error(`no element matches ${selector}`);
  return el;
}

// The same as format_size in core/src/size.rs.
export function formatSize(bytes: number): string {
  const units = ["KB", "MB", "GB", "TB", "PB", "EB"];
  if (bytes < 1000) return `${bytes} B`;
  let value = bytes;
  let unit = "B";
  for (const u of units) {
    value /= 1000;
    unit = u;
    if (value < 1000) break;
  }
  return value < 10 ? `${value.toFixed(1)} ${unit}` : `${value.toFixed(0)} ${unit}`;
}

export function formatCount(n: number): string {
  return n.toLocaleString("en-GB");
}

export function percent(part: number, whole: number): string {
  if (whole === 0) return "0%";
  const p = (part / whole) * 100;
  return p < 1 ? "<1%" : `${p.toFixed(0)}%`;
}
