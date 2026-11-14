import { describe, expect, test } from "vitest";
import type { LayoutFile, Page } from "@decka/shared";
import layoutDefault from "@decka/shared/fixtures/layout.default.json";
import { addItem, findFreeSpot, fits, makeItem, movePage, newItemId, NO_SPACE, removeItem, updateItem } from "../src/tabs/layoutOps";

const layout = layoutDefault as unknown as LayoutFile;
// "main" fills all 15 cells; "dev" has exactly two free cells: (4,1) and (3,2).
const main = layout.pages[0] as Page;
const dev = layout.pages[1] as Page;

describe("fits", () => {
  test("accepts a free cell", () => {
    expect(fits(dev, { x: 4, y: 1, w: 1, h: 1 })).toBe(true);
    expect(fits(dev, { x: 3, y: 2, w: 1, h: 1 })).toBe(true);
  });

  test("catches overlap", () => {
    expect(fits(main, { x: 1, y: 1, w: 1, h: 1 })).toBe(false); // b-play
    expect(fits(dev, { x: 3, y: 1, w: 2, h: 1 })).toBe(false); // clips b-taskmgr
  });

  test("catches leaving the grid", () => {
    expect(fits(dev, { x: 4, y: 1, w: 2, h: 1 })).toBe(false);
    expect(fits(dev, { x: 3, y: 3, w: 1, h: 1 })).toBe(false);
    expect(fits(dev, { x: -1, y: 1, w: 1, h: 1 })).toBe(false);
  });

  test("an item does not collide with itself", () => {
    expect(fits(main, { x: 1, y: 1, w: 1, h: 1 }, "b-play")).toBe(true);
  });
});

describe("addItem", () => {
  test("into an occupied cell is an error", () => {
    expect(addItem(main, makeItem("button", 1, 1, "b-new"))).toEqual({ ok: false, error: NO_SPACE });
  });

  test("into a free cell appends the item", () => {
    const result = addItem(dev, makeItem("button", 4, 1, "b-new"));
    expect(result.ok).toBe(true);
    if (result.ok) expect(result.page.items).toHaveLength(dev.items.length + 1);
  });

  test("a big widget needs its whole area", () => {
    expect(addItem(dev, makeItem("statuses", 3, 1, "w-big")).ok).toBe(false);
  });
});

describe("findFreeSpot", () => {
  test("returns the first free cell row by row, or null when the page is full", () => {
    expect(findFreeSpot(dev, 1, 1)).toEqual({ x: 4, y: 1 });
    expect(findFreeSpot(dev, 2, 1)).toBeNull();
    expect(findFreeSpot(main, 1, 1)).toBeNull();
  });
});

describe("updateItem / removeItem", () => {
  test("change only the target", () => {
    const moved = updateItem(dev, "b-home", { x: 4, y: 1 });
    expect(moved.items.find((i) => i.id === "b-home")).toMatchObject({ x: 4, y: 1 });
    expect(moved.items.find((i) => i.id === "b-github")).toMatchObject({ x: 3, y: 0 });
    expect(removeItem(main, "b-play").items.some((i) => i.id === "b-play")).toBe(false);
  });
});

describe("movePage", () => {
  test("swaps neighbours and ignores out-of-range moves", () => {
    expect(movePage(layout, 0, 1).pages.map((p) => p.id)).toEqual(["dev", "main"]);
    expect(movePage(layout, 1, -1).pages.map((p) => p.id)).toEqual(["dev", "main"]);
    expect(movePage(layout, 0, -1)).toBe(layout);
    expect(movePage(layout, 1, 1)).toBe(layout);
  });
});

test("new ids look like b-xxxxxx and w-xxxxxx", () => {
  expect(newItemId("button")).toMatch(/^b-[a-z0-9]{6}$/);
  expect(newItemId("clock")).toMatch(/^w-[a-z0-9]{6}$/);
});
