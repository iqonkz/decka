import type { LayoutFile, LayoutItem, Page } from "@decka/shared";

export const COLS = 5;
export const ROWS = 3;

export type Kind = LayoutItem["kind"];

/** Size of a newly created item of each kind. */
export const DEFAULT_SIZE: Record<Kind, { w: number; h: number }> = {
  button: { w: 1, h: 1 },
  clock: { w: 2, h: 1 },
  sys: { w: 2, h: 1 },
  status: { w: 1, h: 1 },
  statuses: { w: 2, h: 2 },
  notifications: { w: 2, h: 2 },
};

/** Allowed sizes per kind (DATA_MODEL.md 3.2 item 4): [minW, maxW, minH, maxH]. */
export const SIZE_RANGE: Record<Kind, [number, number, number, number]> = {
  button: [1, 1, 1, 1],
  clock: [1, 2, 1, 1],
  sys: [2, 2, 1, 2],
  status: [1, 1, 1, 1],
  statuses: [2, 5, 1, 3],
  notifications: [2, 5, 1, 3],
};

interface Rect {
  x: number;
  y: number;
  w: number;
  h: number;
}

const overlap = (a: Rect, b: Rect): boolean => a.x < b.x + b.w && b.x < a.x + a.w && a.y < b.y + b.h && b.y < a.y + a.h;

/** True if the rectangle lies inside the 5x3 grid and overlaps no other item of the page. */
export function fits(page: Page, rect: Rect, ignoreId?: string): boolean {
  if (rect.x < 0 || rect.y < 0 || rect.w < 1 || rect.h < 1 || rect.x + rect.w > COLS || rect.y + rect.h > ROWS) return false;
  return page.items.every((item) => item.id === ignoreId || !overlap(rect, item));
}

/** First free position (row by row) for a w x h item, or null. */
export function findFreeSpot(page: Page, w: number, h: number): { x: number; y: number } | null {
  for (let y = 0; y + h <= ROWS; y++) {
    for (let x = 0; x + w <= COLS; x++) {
      if (fits(page, { x, y, w, h })) return { x, y };
    }
  }
  return null;
}

export type AddResult = { ok: true; page: Page } | { ok: false; error: string };

export const NO_SPACE = "Not enough free space here";

export function addItem(page: Page, item: LayoutItem): AddResult {
  if (!fits(page, item)) return { ok: false, error: NO_SPACE };
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
  }
}

/** Page ids: `p-xxxxxx`. */
export function newPageId(random: () => number = Math.random): string {
  return `p-${newItemId("sys", random).slice(2)}`;
}
