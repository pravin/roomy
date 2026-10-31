// Draws the treemap that Rust lays out. Rust does the maths; all we do
// here is paint rectangles.

import * as api from "./api";
import type { NodeId, Tile } from "./api";

// Every file is the same colour, for now.
export const FILE_COLOUR = "#4c8ee8";

const dark = window.matchMedia("(prefers-color-scheme: dark)");

export class Treemap {
  private ctx: CanvasRenderingContext2D;
  private tiles: Tile[] = [];
  private folderId: NodeId | null = null;

  constructor(readonly canvas: HTMLCanvasElement) {
    this.ctx = canvas.getContext("2d")!;
    // Lay out again whenever the window changes size.
    new ResizeObserver(() => this.reload()).observe(canvas.parentElement!);
  }

  async show(id: NodeId) {
    this.folderId = id;
    await this.reload();
  }

  async reload() {
    if (this.folderId === null) return;
    const { width, height } = this.canvas.parentElement!.getBoundingClientRect();
    if (width === 0 || height === 0) return; // Hidden, nothing to draw
    // A canvas has its own pixel size. On a high DPI screen we make it
    // bigger than it looks, so it isn't blurry.
    const ratio = window.devicePixelRatio || 1;
    this.canvas.width = Math.round(width * ratio);
    this.canvas.height = Math.round(height * ratio);
    this.canvas.style.width = `${width}px`;
    this.canvas.style.height = `${height}px`;

    this.tiles = await api.treemap(this.folderId, width, height);
    this.draw(ratio);
  }

  private draw(ratio: number) {
    const ctx = this.ctx;
    ctx.setTransform(ratio, 0, 0, ratio, 0, 0);
    ctx.clearRect(0, 0, this.canvas.width, this.canvas.height);
    ctx.font = "600 11px system-ui, -apple-system, sans-serif";
    ctx.textBaseline = "top";

    for (const t of this.tiles) {
      ctx.fillStyle = t.hasChildren ? (dark.matches ? "#3a414b" : "#c9d0d8") : FILE_COLOUR;
      // Leave a one pixel gap so neighbours don't run into each other.
      ctx.fillRect(t.x + 0.5, t.y + 0.5, t.w - 1, t.h - 1);

      if (t.w >= 40 && t.h >= 20) {
        ctx.save();
        ctx.beginPath();
        ctx.rect(t.x, t.y, t.w - 4, t.h);
        ctx.clip();
        ctx.fillStyle = t.hasChildren && !dark.matches ? "#2a313b" : "#ffffff";
        ctx.fillText(t.name, t.x + 6, t.y + 6);
        ctx.restore();
      }
    }
  }
}
