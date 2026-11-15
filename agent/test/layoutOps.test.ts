import { describe, expect, test } from "vitest";
import type { LayoutFile, Page } from "@decka/shared";
import layoutDefault from "@decka/shared/fixtures/layout.default.json";
import { newPopupPage, normalPages, popupPages, tapModeOf, tapModePatch } from "../src/tabs/layoutOps";
import { addItem, findFreeSpot, fits, makeItem, movePage, newItemId, NO_SPACE, normalizeApp, removeItem, sizeRange, updateItem, withApps, withMediaDefaults } from "../src/tabs/layoutOps";

const layout = layoutDefault as unknown as LayoutFile;
const grid = layout.grid; // 7 x 4
// "main" has free cells (3,3), (4,3), (5,3) only; "dev" is free from (4,1) down, except (6,3).
const main = layout.pages[0] as Page;
const dev = layout.pages[1] as Page;

describe("popup groups (M13): On tap modes", () => {
  const opts = { firstActionId: "media-play", firstPageId: "main", firstPopupId: "media-more" };

  test("the mode of a button comes from its fields", () => {
    expect(tapModeOf({ actionId: "a" })).toBe("action");
    expect(tapModeOf({ navigate: "p" })).toBe("page");
    expect(tapModeOf({ popup: "g" })).toBe("popup");
    expect(tapModeOf({ actionId: "a", popup: "g" })).toBe("action-popup");
    expect(tapModeOf({})).toBe("action");
  });

  test("picking a mode sets its fields and clears the others", () => {
    expect(tapModePatch({ actionId: "x" }, "page", opts)).toEqual({ actionId: undefined, popup: undefined, confirm: undefined, navigate: "main" });
    expect(tapModePatch({}, "popup", opts)).toEqual({ actionId: undefined, navigate: undefined, confirm: undefined, popup: "media-more" });
    expect(tapModePatch({ actionId: "x" }, "action-popup", opts)).toEqual({ navigate: undefined, actionId: "x", popup: "media-more" });
    expect(tapModePatch({}, "action", opts)).toEqual({ navigate: undefined, popup: undefined, actionId: "media-play" });
  });

  test("a new popup group is a popup page named Group N, and the page lists split by kind", () => {
    const pages: Page[] = [{ id: "main", name: "Main", items: [] }, { id: "g1", name: "Group 1", popup: true, items: [] }];
    const group = newPopupPage(pages);
    expect(group).toMatchObject({ name: "Group 2", popup: true, items: [] });
    expect(normalPages(pages).map((p) => p.id)).toEqual(["main"]);
    expect(popupPages(pages).map((p) => p.id)).toEqual(["g1"]);
  });
});

describe("context page helpers (M11)", () => {
  test("normalizeApp lower-cases and appends .exe", () => {
    expect(normalizeApp("Code")).toBe("code.exe");
    expect(normalizeApp("  Chrome.EXE ")).toBe("chrome.exe");
    expect(normalizeApp("   ")).toBeNull();
  });

  test("withApps drops the field when the list is empty", () => {
    expect(withApps(dev, ["code.exe"]).apps).toEqual(["code.exe"]);
    expect("apps" in withApps(dev, [])).toBe(false);
  });

  test("a new media widget gets the standard media actions that exist", () => {
    const media = makeItem("media", 3, 3, "w-test");
    expect(media).toMatchObject({ kind: "media", w: 3, h: 1 });
    expect(withMediaDefaults(media, ["media-prev", "media-play", "media-next"])).toMatchObject({ prevAction: "media-prev", playAction: "media-play", nextAction: "media-next" });
    const partial = withMediaDefaults(media, ["media-play"]);
    expect(partial).toMatchObject({ playAction: "media-play" });
    expect("prevAction" in partial).toBe(false);
  });

  test("media may be 2 to all columns wide and 1 or 2 rows high", () => {
    expect(sizeRange("media", grid)).toEqual([2, 7, 1, 2]);
  });
});

describe("fits (grid 7x4)", () => {
  test("the default grid is 7x4", () => {
    expect(grid).toEqual({ cols: 7, rows: 4 });
  });

  test("accepts a free cell", () => {
    expect(fits(dev, grid, { x: 4, y: 1, w: 1, h: 1 })).toBe(true);
    expect(fits(dev, grid, { x: 6, y: 2, w: 1, h: 1 })).toBe(true);
    expect(fits(main, grid, { x: 3, y: 3, w: 3, h: 1 })).toBe(true);
  });

  test("catches overlap", () => {
    expect(fits(main, grid, { x: 1, y: 2, w: 1, h: 1 })).toBe(false); // b-play
    expect(fits(dev, grid, { x: 3, y: 1, w: 2, h: 1 })).toBe(false); // clips the statuses widget
  });

  test("catches leaving the grid", () => {
    expect(fits(dev, grid, { x: 6, y: 1, w: 2, h: 1 })).toBe(false);
    expect(fits(dev, grid, { x: 4, y: 4, w: 1, h: 1 })).toBe(false);
    expect(fits(dev, grid, { x: -1, y: 1, w: 1, h: 1 })).toBe(false);
  });

  test("follows the grid it is given", () => {
    expect(fits(dev, { cols: 7, rows: 4 }, { x: 6, y: 1, w: 1, h: 1 })).toBe(true);
    expect(fits(dev, { cols: 6, rows: 4 }, { x: 6, y: 1, w: 1, h: 1 })).toBe(false);
    expect(fits(dev, { cols: 8, rows: 5 }, { x: 7, y: 4, w: 1, h: 1 })).toBe(true);
  });

  test("an item does not collide with itself", () => {
    expect(fits(main, grid, { x: 1, y: 2, w: 1, h: 1 }, "b-play")).toBe(true);
  });
});

describe("addItem", () => {
  test("into an occupied cell is an error", () => {
    expect(addItem(main, grid, makeItem("button", 1, 2, "b-new"))).toEqual({ ok: false, error: NO_SPACE });
  });

  test("into a free cell appends the item", () => {
    const result = addItem(dev, grid, makeItem("button", 4, 1, "b-new"));
    expect(result.ok).toBe(true);
    if (result.ok) expect(result.page.items).toHaveLength(dev.items.length + 1);
  });

  test("a big widget needs its whole area", () => {
    expect(addItem(dev, grid, makeItem("statuses", 3, 1, "w-big")).ok).toBe(false);
    expect(addItem(dev, grid, makeItem("notifications", 4, 1, "w-notifs")).ok).toBe(true);
  });
});

describe("findFreeSpot", () => {
  test("returns the first free cell row by row, or null when nothing fits", () => {
    expect(findFreeSpot(dev, grid, 1, 1)).toEqual({ x: 4, y: 1 });
    expect(findFreeSpot(dev, grid, 4, 1)).toBeNull();
    expect(findFreeSpot(main, grid, 1, 1)).toEqual({ x: 3, y: 3 });
    expect(findFreeSpot(main, grid, 4, 1)).toBeNull();
    expect(findFreeSpot(main, { cols: 3, rows: 2 }, 1, 1)).toBeNull();
  });
});

describe("sizeRange", () => {
  test("list widgets may fill the grid, others have fixed limits", () => {
    expect(sizeRange("statuses", { cols: 7, rows: 4 })).toEqual([2, 7, 1, 4]);
    expect(sizeRange("notifications", { cols: 8, rows: 5 })).toEqual([2, 8, 1, 5]);
    expect(sizeRange("button", grid)).toEqual([1, 1, 1, 1]);
    expect(sizeRange("sys", grid)).toEqual([2, 2, 1, 2]);
  });
});

describe("updateItem / removeItem", () => {
  test("change only the target", () => {
    const moved = updateItem(dev, "b-home", { x: 4, y: 1 });
    expect(moved.items.find((i) => i.id === "b-home")).toMatchObject({ x: 4, y: 1 });
    expect(moved.items.find((i) => i.id === "b-github")).toMatchObject({ x: 4, y: 0 });
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
