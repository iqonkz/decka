import { describe, expect, test } from "vitest";
import type { Page } from "@decka/shared";
import { navigableIndices, navigablePosition, placePopup, POPUP_GAP_PX, type Rect } from "../src/deck/popup";

const viewport = { width: 915, height: 412 };
const panel = { width: 300, height: 120 };
const key = (x: number, y: number): Rect => ({ x, y, width: 90, height: 90 });
const inside = (p: { left: number; top: number }, size = panel, pad = 12) =>
  p.left >= pad && p.top >= pad && p.left + size.width <= viewport.width - pad && p.top + size.height <= viewport.height - pad;

describe("placePopup", () => {
  test("centred above the key when there is room", () => {
    const p = placePopup(key(400, 250), panel, viewport, 12);
    expect(p.left).toBe(400 + 45 - 150);
    expect(p.top).toBe(250 - 120 - POPUP_GAP_PX);
    expect(inside(p)).toBe(true);
  });

  test("below the key when the top is too close to the edge", () => {
    const p = placePopup(key(400, 20), panel, viewport, 12);
    expect(p.top).toBe(20 + 90 + POPUP_GAP_PX);
    expect(inside(p)).toBe(true);
  });

  test("always inside the screen, in all four corners and the middle", () => {
    const corners = [key(0, 0), key(825, 0), key(0, 322), key(825, 322), key(400, 160)];
    for (const anchor of corners) {
      expect(inside(placePopup(anchor, panel, viewport, 12)), JSON.stringify(anchor)).toBe(true);
    }
  });

  test("a panel as big as the screen is pinned to the padding", () => {
    const big = { width: 2000, height: 1000 };
    expect(placePopup(key(400, 160), big, viewport, 12)).toEqual({ left: 12, top: 12 });
  });
});

describe("navigable pages", () => {
  const pages = [{ id: "a" }, { id: "b" }, { id: "p", popup: true }, { id: "c" }] as unknown as Page[];
  test("popup pages are skipped by swipe and dots", () => {
    expect(navigableIndices(pages)).toEqual([0, 1, 3]);
    expect(navigablePosition(pages, 3)).toBe(2);
    expect(navigablePosition(pages, 2)).toBe(0);
  });
});
