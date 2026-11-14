export interface GridMetrics {
  /** Side of one square key in px. */
  side: number;
  /** Gap between keys in px (about 4% of the side, at least 4). */
  gap: number;
  /** Free space left of / above the centered grid. */
  offsetX: number;
  offsetY: number;
}

/**
 * Square keys with a small gap, centered on the screen (DATA_MODEL.md 6, T11.10).
 * The margin is 2% of the short side plus 6 px (room for the pixel shift).
 */
export function gridMetrics(viewport: { w: number; h: number }, cols: number, rows: number): GridMetrics {
  const { w, h } = viewport;
  const pad = Math.round(0.02 * Math.min(w, h)) + 6;
  const side0 = Math.min((w - 2 * pad) / cols, (h - 2 * pad) / rows);
  const gap = Math.max(4, Math.round(0.04 * side0));
  const side = Math.max(1, Math.floor(Math.min((w - 2 * pad - (cols - 1) * gap) / cols, (h - 2 * pad - (rows - 1) * gap) / rows)));
  const gridW = cols * side + (cols - 1) * gap;
  const gridH = rows * side + (rows - 1) * gap;
  return { side, gap, offsetX: Math.max(0, Math.floor((w - gridW) / 2)), offsetY: Math.max(0, Math.floor((h - gridH) / 2)) };
}

/** Size in px of an item spanning `w` x `h` cells. */
export function spanSize(m: Pick<GridMetrics, "side" | "gap">, w: number, h: number): { width: number; height: number } {
  return { width: w * m.side + (w - 1) * m.gap, height: h * m.side + (h - 1) * m.gap };
}
