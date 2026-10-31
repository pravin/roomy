// Draws the treemap that Rust lays out, and works out which box the mouse
// is over. Rust does the maths; all we do here is paint rectangles.

import * as api from "./api";
import type { NodeId, Tile } from "./api";
import { formatSize } from "./dom";

// Every file is the same colour, for now.
export const FILE_COLOUR = "#4c8ee8";

const dark = window.matchMedia("(prefers-color-scheme: dark)");
// These must match `header`, `LABEL_WIDTH` and `LABEL_HEIGHT` in
// core/src/treemap.rs, which decide which folders have room for a name.
const HEADER = 16;
const LABEL_WIDTH = 64;
const LABEL_HEIGHT = 40;

// Folders get a little lighter (or darker, in dark mode) at each level, so
// you can see where one ends and the next begins.
function folderColour(depth: number): string {
  const lightness = dark.matches ? 15 + depth * 4 : 92 - depth * 4;
  return `hsl(214 16% ${lightness}%)`;
}

export class Treemap {
  private ctx: CanvasRenderingContext2D;
  // Everything that doesn't change when the mouse moves is drawn once,
  // here, and copied to the screen in one go.
  private base = document.createElement("canvas");
  private tiles: Tile[] = [];
  private folderId: NodeId | null = null;
  private hovered: Tile | null = null;
  private resizeTimer = 0;
  selected: NodeId | null = null;

  // Set these to hear about the mouse.
  onHover: (tile: Tile | null, x: number, y: number) => void = () => {};
  onSelect: (tile: Tile | null) => void = () => {};
  onOpen: (tile: Tile) => void = () => {};

  constructor(readonly canvas: HTMLCanvasElement) {
    this.ctx = canvas.getContext("2d")!;

    canvas.addEventListener("mousemove", (e) => {
      const tile = this.hit(e.offsetX, e.offsetY);
      if (tile !== this.hovered) {
        this.hovered = tile;
        this.draw();
      }
      this.onHover(tile, e.offsetX, e.offsetY);
    });
    canvas.addEventListener("mouseleave", () => {
      this.hovered = null;
      this.draw();
      this.onHover(null, 0, 0);
    });
    canvas.addEventListener("click", (e) => {
      const tile = this.hit(e.offsetX, e.offsetY);
      this.selected = tile?.id ?? null;
      this.draw();
      this.onSelect(tile);
    });
    canvas.addEventListener("dblclick", (e) => {
      const tile = this.hit(e.offsetX, e.offsetY);
      if (tile) this.onOpen(tile);
    });

    // Lay out again when the window changes size, but not on every pixel.
    new ResizeObserver(() => {
      clearTimeout(this.resizeTimer);
      this.resizeTimer = window.setTimeout(() => this.reload(), 120);
    }).observe(canvas.parentElement!);
    dark.addEventListener("change", () => this.paintBase());
  }

  async show(id: NodeId) {
    this.folderId = id;
    this.selected = null;
    await this.reload();
  }

  async reload() {
    if (this.folderId === null) return;
    const { width, height } = this.canvas.parentElement!.getBoundingClientRect();
    if (width === 0 || height === 0) return; // Hidden, nothing to draw
    // A canvas has its own pixel size. On a high DPI screen we make it
    // bigger than it looks, so it isn't blurry.
    const ratio = window.devicePixelRatio || 1;
    for (const c of [this.canvas, this.base]) {
      c.width = Math.round(width * ratio);
      c.height = Math.round(height * ratio);
    }
    this.canvas.style.width = `${width}px`;
    this.canvas.style.height = `${height}px`;

    this.tiles = await api.treemap(this.folderId, width, height);
    this.hovered = null;
    this.paintBase();
  }

  // The deepest box under a point. Children come after their parents in
  // the list, so the last match wins.
  private hit(x: number, y: number): Tile | null {
    for (let i = this.tiles.length - 1; i >= 0; i--) {
      const t = this.tiles[i];
      if (x >= t.x && x < t.x + t.w && y >= t.y && y < t.y + t.h) return t;
    }
    return null;
  }

  private paintBase() {
    const ctx = this.base.getContext("2d")!;
    const ratio = this.base.width / Math.max(this.canvas.clientWidth, 1);
    ctx.setTransform(1, 0, 0, 1, 0, 0);
    ctx.clearRect(0, 0, this.base.width, this.base.height);
    ctx.setTransform(ratio, 0, 0, ratio, 0, 0);
    ctx.font = "11px system-ui, -apple-system, sans-serif";
    ctx.textBaseline = "top";

    for (const t of this.tiles) {
      if (t.hasChildren) this.paintFolder(ctx, t);
      else this.paintFile(ctx, t);
    }
    this.draw();
  }

  private paintFolder(ctx: CanvasRenderingContext2D, t: Tile) {
    ctx.fillStyle = folderColour(t.depth);
    ctx.fillRect(t.x + 0.5, t.y + 0.5, t.w - 1, t.h - 1);
    if (t.w >= LABEL_WIDTH && t.h >= LABEL_HEIGHT) {
      // A slightly darker band along the top, with the folder's name in it.
      ctx.fillStyle = dark.matches ? "rgba(255,255,255,0.05)" : "rgba(0,0,0,0.05)";
      ctx.fillRect(t.x + 0.5, t.y + 0.5, t.w - 1, HEADER);
      this.text(ctx, t, t.name, t.x + 5, t.y + 3, dark.matches ? "#dfe4ea" : "#2a313b", "600 11px");
    }
  }

  private paintFile(ctx: CanvasRenderingContext2D, t: Tile) {
    const colour = FILE_COLOUR;
    // A soft light from the top left gives each box a little depth, like
    // a cushion, which makes neighbours of the same colour easy to tell apart.
    const shade = ctx.createLinearGradient(t.x, t.y, t.x + t.w, t.y + t.h);
    shade.addColorStop(0, mix(colour, "#ffffff", 0.22));
    shade.addColorStop(1, mix(colour, "#000000", 0.18));
    ctx.fillStyle = shade;
    ctx.beginPath();
    ctx.roundRect(t.x + 0.5, t.y + 0.5, t.w - 1, t.h - 1, Math.min(3, t.w / 4, t.h / 4));
    ctx.fill();
    if (t.w >= 48 && t.h >= 30) {
      this.text(ctx, t, t.name, t.x + 6, t.y + 6, "rgba(255,255,255,0.96)", "600 11px");
      if (t.h >= 46) {
        this.text(ctx, t, formatSize(t.size), t.x + 6, t.y + 20, "rgba(255,255,255,0.8)", "11px");
      }
    }
  }

  private text(ctx: CanvasRenderingContext2D, t: Tile, text: string, x: number, y: number, colour: string, font: string) {
    ctx.save();
    ctx.beginPath();
    ctx.rect(t.x + 2, t.y, t.w - 6, t.h);
    ctx.clip();
    ctx.font = `${font} system-ui, -apple-system, sans-serif`;
    ctx.fillStyle = colour;
    ctx.fillText(text, x, y);
    ctx.restore();
  }

  // Copies the base picture to the screen, then adds the hover and
  // selection outlines on top.
  draw() {
    const ctx = this.ctx;
    ctx.setTransform(1, 0, 0, 1, 0, 0);
    ctx.clearRect(0, 0, this.canvas.width, this.canvas.height);
    ctx.drawImage(this.base, 0, 0);
    const ratio = this.canvas.width / Math.max(this.canvas.clientWidth, 1);
    ctx.setTransform(ratio, 0, 0, ratio, 0, 0);

    if (this.hovered) {
      const t = this.hovered;
      ctx.fillStyle = "rgba(255, 255, 255, 0.18)";
      ctx.fillRect(t.x + 0.5, t.y + 0.5, t.w - 1, t.h - 1);
    }
    const selected = this.tiles.find((t) => t.id === this.selected);
    if (selected) {
      ctx.strokeStyle = dark.matches ? "#ffffff" : "#111418";
      ctx.lineWidth = 2;
      ctx.strokeRect(selected.x + 1.5, selected.y + 1.5, selected.w - 3, selected.h - 3);
    }
  }
}

// Mixes two #rrggbb colours. `amount` 0 gives `a`, 1 gives `b`.
function mix(a: string, b: string, amount: number): string {
  const pa = parseInt(a.slice(1), 16);
  const pb = parseInt(b.slice(1), 16);
  const channel = (shift: number) => {
    const ca = (pa >> shift) & 255;
    const cb = (pb >> shift) & 255;
    return Math.round(ca + (cb - ca) * amount);
  };
  return `rgb(${channel(16)}, ${channel(8)}, ${channel(0)})`;
}
