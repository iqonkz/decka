import type { LayoutFile, LayoutItem, Page } from "@decka/shared";

/** Grid size of the layout being edited (DATA_MODEL.md 3.1: cols 3..8, rows 2..5). */
export interface Grid {
  cols: number;
  rows: number;
}
export const COLS_RANGE = [3, 8] as const;
export const ROWS_RANGE = [2, 5] as const;

export type Kind = LayoutItem["kind"];

/** Size of a newly created item of each kind. */
export const DEFAULT_SIZE: Record<Kind, { w: number; h: number }> = {
  button: { w: 1, h: 1 },
  clock: { w: 2, h: 1 },
  sys: { w: 2, h: 1 },
  status: { w: 1, h: 1 },
  statuses: { w: 2, h: 2 },
  notifications: { w: 2, h: 2 },
  media: { w: 3, h: 1 },
};

/** Allowed sizes per kind (DATA_MODEL.md 3.2 item 4): [minW, maxW, minH, maxH]; list widgets may fill the grid. */
export function sizeRange(kind: Kind, grid: Grid): [number, number, number, number] {
  switch (kind) {
    case "button":
    case "status":
      return [1, 1, 1, 1];
    case "clock":
      return [1, 2, 1, 1];
    case "sys":
      return [2, 2, 1, 2];
    case "statuses":
    case "notifications":
      return [2, grid.cols, 1, grid.rows];
    case "media":
      return [2, grid.cols, 1, 2];
  }
}

interface Rect {
  x: number;
  y: number;
  w: number;
  h: number;
}

const overlap = (a: Rect, b: Rect): boolean => a.x < b.x + b.w && b.x < a.x + a.w && a.y < b.y + b.h && b.y < a.y + a.h;

/** True if the rectangle lies inside the grid and overlaps no other item of the page. */
export function fits(page: Page, grid: Grid, rect: Rect, ignoreId?: string): boolean {
  if (rect.x < 0 || rect.y < 0 || rect.w < 1 || rect.h < 1 || rect.x + rect.w > grid.cols || rect.y + rect.h > grid.rows) return false;
  return page.items.every((item) => item.id === ignoreId || !overlap(rect, item));
}

/** First free position (row by row) for a w x h item, or null. */
export function findFreeSpot(page: Page, grid: Grid, w: number, h: number): { x: number; y: number } | null {
  for (let y = 0; y + h <= grid.rows; y++) {
    for (let x = 0; x + w <= grid.cols; x++) {
      if (fits(page, grid, { x, y, w, h })) return { x, y };
    }
  }
  return null;
}

export type AddResult = { ok: true; page: Page } | { ok: false; error: string };

export const NO_SPACE = "Not enough free space here";

export function addItem(page: Page, grid: Grid, item: LayoutItem): AddResult {
  if (!fits(page, grid, item)) return { ok: false, error: NO_SPACE };
  return { ok: true, page: { ...page, items: [...page.items, item] } };
}

/** Shallow merge of `patch` into the item with that id (validity is checked later by validateLayout). */
export function updateItem(page: Page, id: string, patch: Partial<LayoutItem>): Page {
  return { ...page, items: page.items.map((i) => (i.id === id ? ({ ...i, ...patch } as LayoutItem) : i)) };
}

export function removeItem(page: Page, id: string): Page {
  return { ...page, items: page.items.filter((i) => i.id !== id) };
}

/** Moves a page by `delta` positions (-1 = left, +1 = right); out-of-range moves change nothing. */
export function movePage(layout: LayoutFile, index: number, delta: number): LayoutFile {
  const target = index + delta;
  if (index < 0 || index >= layout.pages.length || target < 0 || target >= layout.pages.length) return layout;
  const pages = [...layout.pages];
  const [moved] = pages.splice(index, 1);
  if (!moved) return layout;
  pages.splice(target, 0, moved);
  return { ...layout, pages };
}

/** `b-xxxxxx` for buttons, `w-xxxxxx` for widgets. */
export function newItemId(kind: Kind, random: () => number = Math.random): string {
  const chars = "abcdefghijklmnopqrstuvwxyz0123456789";
  let suffix = "";
  for (let i = 0; i < 6; i++) suffix += chars[Math.floor(random() * chars.length)] ?? "a";
  return `${kind === "button" ? "b" : "w"}-${suffix}`;
}

/** A new item of `kind` with its default size at (x, y). */
export function makeItem(kind: Kind, x: number, y: number, id = newItemId(kind)): LayoutItem {
  const { w, h } = DEFAULT_SIZE[kind];
  const base = { id, x, y, w, h };
  switch (kind) {
    case "button":
      return { ...base, kind, label: "New" };
    case "clock":
      return { ...base, kind };
    case "sys":
      return { ...base, kind };
    case "status":
      return { ...base, kind, statusId: "" };
    case "statuses":
      return { ...base, kind };
    case "notifications":
      return { ...base, kind };
    case "media":
      return { ...base, kind };
  }
}

/** Page ids: `p-xxxxxx`. */
export function newPageId(random: () => number = Math.random): string {
  return `p-${newItemId("sys", random).slice(2)}`;
}
