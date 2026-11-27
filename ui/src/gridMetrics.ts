import type { LayoutItem } from "@decka/shared";

export interface GridMetrics {
  side: number;
  cellW: number;
  cellH: number;
  gap: number;
  offsetX: number;
  offsetY: number;
}

export const GRID_GAP = 8;
export const GRID_PAD = 8;

/**
 * Full-screen stretch grid (M31 T31.7b): 8 px padding and 8 px gap on every aspect ratio
 * (16:9, 18.5:9, 20:9, 4:3), cells stretch to fill width and height without empty side bars.
 * `side` stays `min(cellW, cellH)` for icon/label sizing inside rectangular tiles.
 */
export function gridMetrics(viewport: { w: number; h: number }, cols: number, rows: number): GridMetrics {
  const c = Math.max(1, cols);
  const r = Math.max(1, rows);
  const w = Math.max(1, viewport.w);
  const h = Math.max(1, viewport.h);
  const gap = GRID_GAP;
  const offsetX = GRID_PAD;
  const offsetY = GRID_PAD;
  const availW = Math.max(c, w - 2 * offsetX - (c - 1) * gap);
  const availH = Math.max(r, h - 2 * offsetY - (r - 1) * gap);
  const cellW = availW / c;
  const cellH = availH / r;
  const side = Math.max(1, Math.min(cellW, cellH));
  return { side, cellW, cellH, gap, offsetX, offsetY };
}

export function spanSize(m: Pick<GridMetrics, "side" | "gap"> & { cellW?: number; cellH?: number }, w: number, h: number): { width: number; height: number } {
  const cw = m.cellW ?? m.side;
  const ch = m.cellH ?? m.side;
  return { width: w * cw + (w - 1) * m.gap, height: h * ch + (h - 1) * m.gap };
}

/**
 * Transpose a grid's items when rotating portrait <-> landscape (e.g. 7x4 -> 4x7),
 * keeping items in linear reading order (row by row, left to right) without overlaps.
 */
export function transposeItemsForGrid(
  items: readonly LayoutItem[],
  srcCols: number,
  srcRows: number,
  dstCols: number,
  dstRows: number,
): LayoutItem[] {
  if (srcCols === dstCols && srcRows === dstRows) return items as LayoutItem[];
  const ordered = [...items].sort((a, b) => a.y - b.y || a.x - b.x || a.id.localeCompare(b.id));
  const used = Array.from({ length: dstRows }, () => Array<boolean>(dstCols).fill(false));
  const out: LayoutItem[] = [];

  const fitsAt = (x: number, y: number, w: number, h: number) => {
    if (x + w > dstCols || y + h > dstRows) return false;
    for (let dy = 0; dy < h; dy++) {
      for (let dx = 0; dx < w; dx++) {
        if (used[y + dy]![x + dx]) return false;
      }
    }
    return true;
  };

  const occupy = (x: number, y: number, w: number, h: number) => {
    for (let dy = 0; dy < h; dy++) {
      for (let dx = 0; dx < w; dx++) {
        used[y + dy]![x + dx] = true;
      }
    }
  };

  for (const item of ordered) {
    const w = Math.max(1, Math.min(item.w, dstCols));
    const h = Math.max(1, Math.min(item.h, dstRows));
    let placed = false;
    for (let y = 0; y < dstRows && !placed; y++) {
      for (let x = 0; x < dstCols && !placed; x++) {
        if (fitsAt(x, y, w, h)) {
          occupy(x, y, w, h);
          out.push({ ...item, x, y, w, h });
          placed = true;
        }
      }
    }
    // No room for its size: it shrinks to the largest rectangle that is still free, never onto another tile.
    if (!placed) {
      const sizes: [number, number][] = [];
      for (let sw = w; sw >= 1; sw--) for (let sh = h; sh >= 1; sh--) sizes.push([sw, sh]);
      sizes.sort((a, b) => b[0] * b[1] - a[0] * a[1]);
      for (const [sw, sh] of sizes) {
        for (let y = 0; y < dstRows && !placed; y++) {
          for (let x = 0; x < dstCols && !placed; x++) {
            if (fitsAt(x, y, sw, sh)) {
              occupy(x, y, sw, sh);
              out.push({ ...item, x, y, w: sw, h: sh });
              placed = true;
            }
          }
        }
        if (placed) break;
      }
    }
    // A cell is always free while the tiles fit the source grid; a layout with more tiles than cells loses none silently.
    if (!placed) out.push({ ...item, x: 0, y: dstRows - 1, w: 1, h: 1 });
  }
  return out;
}
