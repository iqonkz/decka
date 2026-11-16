import type { LayoutFile, LayoutItem, Orientation, Page } from "./layout";
import { gridLimits, sizeLimits } from "./validate";

export type RotateResult =
  | { ok: true; layout: LayoutFile; /** Items that had no place in the turned page and went to a new page. */ moved: number }
  | { ok: false; error: string };

const MAX_PAGES = 10;
const MAX_NAME = 30;

const clamp = (n: number, [min, max]: [number, number]): number => Math.min(max, Math.max(min, n));

/** Whether a `w` x `h` item at (x, y) lies inside the grid and clear of `taken`. */
function free(taken: LayoutItem[], grid: { cols: number; rows: number }, x: number, y: number, w: number, h: number): boolean {
  if (x < 0 || y < 0 || x + w > grid.cols || y + h > grid.rows) return false;
  return !taken.some((o) => x < o.x + o.w && o.x < x + w && y < o.y + o.h && o.y < y + h);
}

/** The first spot, top to bottom and left to right, where a `w` x `h` item fits. */
function firstFree(taken: LayoutItem[], grid: { cols: number; rows: number }, w: number, h: number): { x: number; y: number } | null {
  for (let y = 0; y + h <= grid.rows; y++) for (let x = 0; x + w <= grid.cols; x++) if (free(taken, grid, x, y, w, h)) return { x, y };
  return null;
}

/** `<name> 2`, `<name> 3`...: the name is cut so that the whole stays within 30 characters. */
function overflowName(name: string, n: number): string {
  const suffix = ` ${n}`;
  return name.slice(0, MAX_NAME - suffix.length) + suffix;
}

/**
 * The layout turned to `to` (DATA_MODEL.md 3.7, revised 2026-11-16). The grid is transposed and clamped to the limits of the
 * new orientation; the content flows into it keeping the reading order: widgets first, then buttons, each group by (y, x) of
 * the old page, every item into the first free spot top to bottom, left to right. A widget keeps its size (clamped to the new
 * grid); a horizontal `volume` becomes vertical (w and h swap). Whatever does not fit goes to a new page `<name> 2` after the
 * page. A popup group that does not fit is an error and nothing changes.
 */
export function rotateLayout(layout: LayoutFile, to: Orientation): RotateResult {
  const from: Orientation = layout.display.orientation === "portrait" ? "portrait" : "landscape";
  if (from === to) return { ok: true, layout, moved: 0 };

  const limits = gridLimits(to);
  const grid = { cols: clamp(layout.grid.rows, limits.cols), rows: clamp(layout.grid.cols, limits.rows) };
  const usedIds = new Set(layout.pages.map((p) => p.id));
  const pages: Page[] = [];
  let moved = 0;

  for (const page of layout.pages) {
    const order = [...page.items].sort((a, b) => {
      const widgetsFirst = Number(a.kind === "button") - Number(b.kind === "button");
      return widgetsFirst || a.y - b.y || a.x - b.x;
    });

    const placed: LayoutItem[] = [];
    const extra: LayoutItem[][] = []; // items of the new pages `<name> 2`, `<name> 3`...
    for (const item of order) {
      // `volume` is not in the size table yet (T15.1): its sizes are 1x2..1x4 vertical or 2x1..4x1 horizontal.
      const isVolume = (item.kind as string) === "volume";
      const [minW, maxW, minH, maxH] = isVolume ? [1, Math.min(4, grid.cols), 1, Math.min(4, grid.rows)] : (sizeLimits(item.kind, grid) ?? [1, 1, 1, 1]);
      // A widget keeps its size, cut down to the new grid; a button is always 1x1; a horizontal volume turns vertical.
      const turned = isVolume && item.w > item.h ? { w: item.h, h: item.w } : { w: item.w, h: item.h };
      const w = item.kind === "button" ? 1 : Math.min(turned.w, maxW);
      const h = item.kind === "button" ? 1 : Math.min(turned.h, maxH);
      const tooSmall = w < minW || h < minH;
      const spot = tooSmall ? null : firstFree(placed, grid, w, h);
      if (spot) {
        placed.push({ ...item, x: spot.x, y: spot.y, w, h } as LayoutItem);
        continue;
      }
      if (page.popup === true) return { ok: false, error: `Popup group "${page.name}" doesn't fit. Make it smaller first.` };
      // No place on this page: the first new page of this one with room, a fresh one when all are full.
      const fits = tooSmall ? [] : extra.map((list) => firstFree(list, grid, w, h));
      const into = fits.findIndex((s) => s !== null);
      const size = { w: Math.max(w, minW), h: Math.max(h, minH) };
      if (into >= 0) {
        const at = fits[into]!;
        extra[into]!.push({ ...item, x: at.x, y: at.y, w: size.w, h: size.h } as LayoutItem);
      } else {
        const at = firstFree([], grid, size.w, size.h);
        if (!at) return { ok: false, error: `"${page.name}" has an item that cannot fit the turned grid.` };
        extra.push([{ ...item, x: at.x, y: at.y, w: size.w, h: size.h } as LayoutItem]);
      }
      moved++;
    }

    pages.push({ ...page, items: placed });
    extra.forEach((items, i) => {
      let id = `${page.id}-${i + 2}`.slice(0, 64);
      for (let n = 2; usedIds.has(id); n++) id = `${page.id}-r${n}`.slice(0, 64);
      usedIds.add(id);
      // DATA_MODEL.md 3.7 says the new page keeps `apps`, but rule 9 of 3.2 lets an app belong to one page only, and the result
      // must be valid: the new page has no `apps` (it is a normal page one swipe away). See BLOCKERS.md, T14.9.
      pages.push({ id, name: overflowName(page.name, i + 2), items });
    });
  }
  if (pages.length > MAX_PAGES) return { ok: false, error: `Turning the layout would make ${pages.length} pages; the limit is ${MAX_PAGES}.` };

  return { ok: true, layout: { ...layout, grid, display: { ...layout.display, orientation: to }, pages }, moved };
}
