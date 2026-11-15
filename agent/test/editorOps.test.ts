import { describe, expect, test } from "vitest";
import type { LayoutItem, Page } from "@decka/shared";
import { canRedo, canUndo, dragSize, HISTORY_LIMIT, itemAt, moveItem, nearestFreeCell, newHistory, pushHistory, redo, resizeItem, swapItems, undo } from "../src/tabs/editorOps";

const grid = { cols: 5, rows: 3 };
const btn = (id: string, x: number, y: number): LayoutItem => ({ id, kind: "button", x, y, w: 1, h: 1, label: id, actionId: "a" });
const page = (items: LayoutItem[]): Page => ({ id: "p", name: "P", items });
const pos = (p: Page | null, id: string) => {
  const i = p?.items.find((x) => x.id === id);
  return i ? [i.x, i.y] : null;
};

describe("moveItem", () => {
  const p = page([btn("a", 0, 0), btn("b", 1, 0), { id: "w", kind: "sys", x: 3, y: 0, w: 2, h: 2 }]);

  test("moves to a free cell", () => {
    expect(pos(moveItem(p, grid, "a", 0, 2), "a")).toEqual([0, 2]);
  });

  test("onto an occupied cell it swaps with that item", () => {
    const moved = moveItem(p, grid, "a", 1, 0);
    expect(pos(moved, "a")).toEqual([1, 0]);
    expect(pos(moved, "b")).toEqual([0, 0]);
  });

  test("outside the grid, onto itself, or an unknown id changes nothing (null)", () => {
    expect(moveItem(p, grid, "a", 5, 0)).toBeNull();
    expect(moveItem(p, grid, "a", 0, 0)).toBeNull();
    expect(moveItem(p, grid, "nope", 1, 1)).toBeNull();
  });

  test("a wide widget does not move where it would overlap or leave the grid", () => {
    expect(moveItem(p, grid, "w", 4, 0)).toBeNull();
    expect(pos(moveItem(p, grid, "w", 2, 1), "w")).toEqual([2, 1]);
  });
});

describe("swapItems", () => {
  test("two buttons exchange places", () => {
    const swapped = swapItems(page([btn("a", 0, 0), btn("b", 2, 2)]), grid, "a", "b");
    expect(pos(swapped, "a")).toEqual([2, 2]);
    expect(pos(swapped, "b")).toEqual([0, 0]);
  });

  test("a button and a wide widget swap only when both still fit", () => {
    const p = page([btn("a", 0, 0), { id: "w", kind: "sys", x: 1, y: 0, w: 2, h: 1 }]);
    expect(swapItems(p, grid, "a", "w")).toBeNull(); // the widget would land on the button's cell and overlap the button's new cell? 2x1 at (0,0) overlaps (1,0)
    const roomy = page([btn("a", 0, 0), { id: "w", kind: "sys", x: 2, y: 2, w: 2, h: 1 }]);
    const swapped = swapItems(roomy, grid, "a", "w");
    expect(pos(swapped, "a")).toEqual([2, 2]);
    expect(pos(swapped, "w")).toEqual([0, 0]);
  });

  test("the same item or a missing one is refused", () => {
    expect(swapItems(page([btn("a", 0, 0)]), grid, "a", "a")).toBeNull();
    expect(swapItems(page([btn("a", 0, 0)]), grid, "a", "z")).toBeNull();
  });
});

describe("resizeItem and dragSize", () => {
  const p = page([{ id: "n", kind: "notifications", x: 0, y: 0, w: 2, h: 1 }, btn("b", 0, 1)]);

  test("grows into free space only", () => {
    expect(resizeItem(p, grid, "n", 3, 1)).not.toBeNull();
    expect(resizeItem(p, grid, "n", 2, 2)).toBeNull(); // the button is in the way
  });

  test("respects the allowed sizes of the kind", () => {
    expect(resizeItem(p, grid, "b", 2, 1)).toBeNull(); // a button is 1x1
    const clock = page([{ id: "c", kind: "clock", x: 0, y: 0, w: 1, h: 1 }]);
    expect(resizeItem(clock, grid, "c", 2, 1)).not.toBeNull();
    expect(resizeItem(clock, grid, "c", 3, 1)).toBeNull();
  });

  test("the same size is not a change", () => {
    expect(resizeItem(p, grid, "n", 2, 1)).toBeNull();
  });

  test("a corner drag is clamped to the allowed range", () => {
    const n = p.items[0] as LayoutItem;
    expect(dragSize(n, grid, 10, 10)).toEqual({ w: 5, h: 3 });
    expect(dragSize(n, grid, -10, -10)).toEqual({ w: 2, h: 1 });
    expect(dragSize(n, grid, 1, 0)).toEqual({ w: 3, h: 1 });
  });
});

describe("itemAt", () => {
  test("finds the item that covers a cell, also inside a wide widget", () => {
    const p = page([{ id: "w", kind: "sys", x: 1, y: 0, w: 2, h: 2 }, btn("a", 0, 0)]);
    expect(itemAt(p, 2, 1)?.id).toBe("w");
    expect(itemAt(p, 0, 0)?.id).toBe("a");
    expect(itemAt(p, 4, 2)).toBeUndefined();
  });
});

describe("history (undo / redo, 50 steps)", () => {
  test("push, undo, redo", () => {
    let h = newHistory(0);
    h = pushHistory(h, 1);
    h = pushHistory(h, 2);
    expect(h.present).toBe(2);
    h = undo(h);
    expect(h.present).toBe(1);
    expect(canRedo(h)).toBe(true);
    h = redo(h);
    expect(h.present).toBe(2);
    expect(canRedo(h)).toBe(false);
  });

  test("a new change forgets redo; an unchanged value is not a step", () => {
    let h = pushHistory(pushHistory(newHistory(0), 1), 2);
    h = undo(h);
    h = pushHistory(h, 9);
    expect(h.future).toEqual([]);
    expect(pushHistory(h, 9)).toBe(h);
  });

  test("undo and redo at the ends do nothing", () => {
    const h = newHistory("a");
    expect(undo(h)).toBe(h);
    expect(redo(h)).toBe(h);
    expect(canUndo(h)).toBe(false);
  });

  test("keeps the last 50 steps", () => {
    let h = newHistory(0);
    for (let i = 1; i <= 80; i++) h = pushHistory(h, i);
    expect(h.past).toHaveLength(HISTORY_LIMIT);
    expect(h.past[0]).toBe(80 - HISTORY_LIMIT);
    for (let i = 0; i < 60; i++) h = undo(h);
    expect(h.present).toBe(80 - HISTORY_LIMIT);
  });
});

describe("nearestFreeCell", () => {
  const p = page([btn("a", 0, 0), btn("b", 1, 0), btn("c", 0, 1)]);
  test("a free target stays, an occupied one goes to the closest free cell", () => {
    expect(nearestFreeCell(p, grid, 3, 1)).toEqual({ x: 3, y: 1 });
    expect(nearestFreeCell(p, grid, 0, 0)).toEqual({ x: 1, y: 1 });
  });
  test("a wide item needs room for its width; a full page has none", () => {
    const wide = nearestFreeCell(p, grid, 0, 0, 2, 1);
    expect(wide).toEqual({ x: 1, y: 1 }); // (1,1)-(2,1) is the nearest free 2x1 slot
    const full = page(Array.from({ length: 15 }, (_, i) => btn(`k${i}`, i % 5, Math.floor(i / 5))));
    expect(nearestFreeCell(full, grid, 2, 1)).toBeNull();
  });
});