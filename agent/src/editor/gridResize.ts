import type { LayoutFile, LayoutItem, Page } from "@decka/shared";
import { findFreeSpot, newPageId } from "../tabs/layoutOps";

export interface Overflow {
  /** Index of the page the item is on. */
  page: number;
  item: LayoutItem;
}

/** The items that stick out of (or do not fit) a `cols` x `rows` grid: they would have to move (DESIGN.md 4.6a). */
export function overflowItems(layout: LayoutFile, cols: number, rows: number): Overflow[] {
  const out: Overflow[] = [];
  layout.pages.forEach((p, page) => {
    for (const item of p.items) if (item.x + item.w > cols || item.y + item.h > rows) out.push({ page, item });
  });
  return out;
}

/**
 * The layout in a `cols` x `rows` grid with the items that no longer fit moved to a new page at the end (`Moved`, or
 * `Moved 2`...), each into the first free spot of its size in reading order. Items that fit stay where they are.
 * `moved` counts them; `lost` counts items too large for the new grid on any page (they cannot be placed; the caller must
 * not offer the move then).
 */
export function resizeGridMoving(layout: LayoutFile, cols: number, rows: number): { layout: LayoutFile; moved: number; lost: number } {
  const stuck = overflowItems(layout, cols, rows);
  const grid = { cols, rows };
  const keep: Page[] = layout.pages.map((p) => ({ ...p, items: p.items.filter((i) => !stuck.some((s) => s.item.id === i.id)) }));
  // Popups keep to their own kind: a popup group's keys are moved into another group page, never into a normal page.
  const extra: Page[] = [];
  let moved = 0;
  let lost = 0;
  const target = (popup: boolean): Page => {
    let page = extra.find((p) => (p.popup === true) === popup);
    if (!page) {
      const n = extra.filter((p) => (p.popup === true) === popup).length + 1;
      page = { id: newPageId(), name: popup ? `Moved group${n > 1 ? ` ${n}` : ""}` : `Moved${n > 1 ? ` ${n}` : ""}`, ...(popup ? { popup: true } : {}), items: [] };
      extra.push(page);
    }
    return page;
  };
  for (const { page, item } of stuck) {
    const popup = layout.pages[page]?.popup === true;
    let page2 = target(popup);
    let spot = findFreeSpot(page2, grid, item.w, item.h);
    if (!spot) {
      // The page is full: another one.
      const n = extra.filter((p) => (p.popup === true) === popup).length + 1;
      page2 = { id: newPageId(), name: `${popup ? "Moved group" : "Moved"} ${n}`, ...(popup ? { popup: true } : {}), items: [] };
      extra.push(page2);
      spot = findFreeSpot(page2, grid, item.w, item.h);
    }
    if (!spot || item.w > cols || item.h > rows) {
      lost++;
      continue;
    }
    page2.items.push({ ...item, x: spot.x, y: spot.y } as LayoutItem);
    moved++;
  }
  return { layout: { ...layout, grid, pages: [...keep, ...extra.filter((p) => p.items.length > 0)] }, moved, lost };
}