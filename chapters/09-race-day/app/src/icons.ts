// A handful of line icons, drawn on a 24 by 24 grid. Each is one SVG path.

const PATHS = {
  folder: "M3 7.5A1.5 1.5 0 0 1 4.5 6H9l2 2h8.5A1.5 1.5 0 0 1 21 9.5v8a1.5 1.5 0 0 1-1.5 1.5h-15A1.5 1.5 0 0 1 3 17.5z",
  file: "M6.5 3H14l4.5 4.5v12a1.5 1.5 0 0 1-1.5 1.5H6.5A1.5 1.5 0 0 1 5 19.5v-15A1.5 1.5 0 0 1 6.5 3zM14 3v4.5h4.5",
  home: "M3.5 11 12 4l8.5 7M6 9.5V20h12V9.5M10 20v-5.5h4V20",
  disk: "M3 14 5.6 6.8A1.2 1.2 0 0 1 6.7 6h10.6a1.2 1.2 0 0 1 1.1.8L21 14M3 14v4a1 1 0 0 0 1 1h16a1 1 0 0 0 1-1v-4zM17 16.5h.01",
  settings: "M4 7h9M17 7h3M4 17h3M11 17h9M15 5v4M9 15v4",
  refresh: "M20 12a8 8 0 1 1-2.3-5.7M20 4v5h-5",
  up: "M12 19V5M5.5 11.5 12 5l6.5 6.5",
  back: "M15 5l-7 7 7 7",
  trash: "M4 7h16M9.5 7V4.5h5V7M6.5 7l1 13h9l1-13M10 11v5M14 11v5",
  reveal: "M10.5 4a6.5 6.5 0 1 0 0 13 6.5 6.5 0 0 0 0-13zM15.5 15.5 20 20",
  open: "M14 4h6v6M20 4l-8.5 8.5M18 14v5a1 1 0 0 1-1 1H5a1 1 0 0 1-1-1V7a1 1 0 0 1 1-1h5",
  map: "M4 4h8v10H4zM14 4h6v6h-6zM14 12h6v8h-6zM4 16h8v4H4z",
  sparkle: "M12 3.5l1.9 4.6 4.6 1.9-4.6 1.9L12 16.5l-1.9-4.6L5.5 10l4.6-1.9zM18.5 15.5l.8 1.7 1.7.8-1.7.8-.8 1.7-.8-1.7-1.7-.8 1.7-.8z",
  copies: "M9 9h10.5v10.5H9zM6 15H4.5V4.5H15V6",
  plus: "M12 5v14M5 12h14",
  close: "M6 6l12 12M18 6 6 18",
  chevron: "M9 6l6 6-6 6",
  warning: "M12 4 2.5 20h19zM12 10v4.5M12 17.5h.01",
} as const;

export type IconName = keyof typeof PATHS;

const SVG = "http://www.w3.org/2000/svg";

export function icon(name: IconName, size = 16): SVGSVGElement {
  const svg = document.createElementNS(SVG, "svg");
  svg.setAttribute("viewBox", "0 0 24 24");
  svg.setAttribute("width", String(size));
  svg.setAttribute("height", String(size));
  svg.setAttribute("fill", "none");
  svg.setAttribute("stroke", "currentColor");
  svg.setAttribute("stroke-width", "1.8");
  svg.setAttribute("stroke-linecap", "round");
  svg.setAttribute("stroke-linejoin", "round");
  svg.setAttribute("aria-hidden", "true");
  svg.classList.add("icon");
  const path = document.createElementNS(SVG, "path");
  path.setAttribute("d", PATHS[name]);
  svg.append(path);
  return svg;
}

// Puts icons into every element with a data-icon attribute, so the static
// HTML can ask for them too.
export function fillIcons(root: ParentNode = document) {
  for (const el of root.querySelectorAll<HTMLElement>("[data-icon]")) {
    if (el.querySelector("svg.icon")) continue;
    el.prepend(icon(el.dataset.icon as IconName, Number(el.dataset.size) || 16));
  }
}
