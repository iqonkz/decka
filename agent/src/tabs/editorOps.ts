import type { LayoutItem, Page } from "@decka/shared";
import { fits, sizeRange, type Grid } from "./layoutOps";

/** The item that covers cell (x, y), if any. */
export function itemAt(page: Page, x: number, y: number): LayoutItem | undefined {
  return page.items.find((i) => x >= i.x && x < i.x + i.w && y >= i.y && y < i.y + i.h);
}

const withItem = (page: Page, id: string, patch: Partial<LayoutItem>): Page => ({
  ...page,
  items: page.items.map((i) => (i.id === id ? ({ ...i, ...patch } as LayoutItem) : i)),
});

/**
 * Drags the item `id` so that its top-left cell lands on (x, y).
 * Free space: the item moves. An occupied target: the item swaps with the one it lands on when both fit afterwards.
 * Anything else (outside the grid, no room): `null`, nothing changes.
 */
export function moveItem(page: Page, grid: Grid, id: string, x: number, y: number): Page | null {
  const item = page.items.find((i) => i.id === id);
  if (!item) return null;
  if (item.x === x && item.y === y) return null;
  if (fits(page, grid, { x, y, w: item.w, h: item.h }, id)) return withItem(page, id, { x, y });
  const target = itemAt(page, x, y);
  if (!target || target.id === id) return null;
  return swapItems(page, grid, id, target.id);
}

/** Exchanges the positions of two items when each fits where the other was (their own old spot excluded). */
export function swapItems(page: Page, grid: Grid, aId: string, bId: string): Page | null {
  const a = page.items.find((i) => i.id === aId);
  const b = page.items.find((i) => i.id === bId);
  if (!a || !b || a.id === b.id) return null;
  const rest: Page = { ...page, items: page.items.filter((i) => i.id !== aId && i.id !== bId) };
  const aMoved = { x: b.x, y: b.y, w: a.w, h: a.h };
  const bMoved = { x: a.x, y: a.y, w: b.w, h: b.h };
  const overlap = (p: typeof aMoved, q: typeof bMoved) => p.x < q.x + q.w && q.x < p.x + p.w && p.y < q.y + q.h && q.y < p.y + p.h;
  if (!fits(rest, grid, aMoved) || !fits(rest, grid, bMoved) || overlap(aMoved, bMoved)) return null;
  return withItem(withItem(page, aId, { x: aMoved.x, y: aMoved.y }), bId, { x: bMoved.x, y: bMoved.y });
}

/** Resizes a widget to w x h cells within its allowed sizes and the free space; `null` when that is not possible. */
export function resizeItem(page: Page, grid: Grid, id: string, w: number, h: number): Page | null {
  const item = page.items.find((i) => i.id === id);
  if (!item) return null;
  const [minW, maxW, minH, maxH] = sizeRange(item.kind, grid);
  if (w < minW || w > maxW || h < minH || h > maxH) return null;
  if (w === item.w && h === item.h) return null;
  if (!fits(page, grid, { x: item.x, y: item.y, w, h }, id)) return null;
  return withItem(page, id, { w, h });
}

/** The size a corner drag of `dxCells`/`dyCells` asks for, clamped to the allowed range. */
export function dragSize(item: LayoutItem, grid: Grid, dxCells: number, dyCells: number): { w: number; h: number } {
  const [minW, maxW, minH, maxH] = sizeRange(item.kind, grid);
  const clamp = (v: number, lo: number, hi: number) => Math.max(lo, Math.min(hi, v));
  return { w: clamp(item.w + dxCells, minW, maxW), h: clamp(item.h + dyCells, minH, maxH) };
}

// ===== undo / redo =====

export interface History<T> {
  past: T[];
  present: T;
  future: T[];
}

/** Steps kept for undo (DESIGN.md 4.4). */
export const HISTORY_LIMIT = 50;

export const newHistory = <T>(present: T): History<T> => ({ past: [], present, future: [] });

/** Makes `next` the present; the old present becomes an undo step, redo is forgotten. Equal values are not steps. */
export function pushHistory<T>(h: History<T>, next: T, same: (a: T, b: T) => boolean = (a, b) => a === b): History<T> {
  if (same(h.present, next)) return h;
  return { past: [...h.past, h.present].slice(-HISTORY_LIMIT), present: next, future: [] };
}

export function undo<T>(h: History<T>): History<T> {
  const previous = h.past[h.past.length - 1];
  if (previous === undefined) return h;
  return { past: h.past.slice(0, -1), present: previous, future: [h.present, ...h.future] };
}

export function redo<T>(h: History<T>): History<T> {
  const [next, ...rest] = h.future;
  if (next === undefined) return h;
  return { past: [...h.past, h.present], present: next, future: rest };
}

export const canUndo = <T>(h: History<T>): boolean => h.past.length > 0;
export const canRedo = <T>(h: History<T>): boolean => h.future.length > 0;
