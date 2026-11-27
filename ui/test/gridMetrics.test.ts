import { describe, expect, test } from "vitest";
import type { LayoutItem } from "@decka/shared";
import { GRID_GAP, GRID_PAD, gridMetrics, spanSize, transposeItemsForGrid } from "../src/gridMetrics";

const fillsViewport = (w: number, h: number, cols: number, rows: number) => {
  const m = gridMetrics({ w, h }, cols, rows);
  expect(m.gap).toBe(GRID_GAP);
  expect(m.offsetX).toBe(GRID_PAD);
  expect(m.offsetY).toBe(GRID_PAD);
  const totalW = cols * m.cellW + (cols - 1) * m.gap;
  const totalH = rows * m.cellH + (rows - 1) * m.gap;
  expect(totalW).toBeCloseTo(w - 2 * GRID_PAD, 5);
  expect(totalH).toBeCloseTo(h - 2 * GRID_PAD, 5);
  expect(m.side).toBeCloseTo(Math.min(m.cellW, m.cellH), 5);
  return m;
};

describe("gridMetrics (T31.7b full-screen stretch + transpose)", () => {
  test("fills 16:9, 18.5:9, 20:9, and 4:3 viewports with 8 px gap and 8 px padding", () => {
    fillsViewport(800, 450, 7, 4); // 16:9
    fillsViewport(848, 412, 7, 4); // 18.5:9
    fillsViewport(915, 412, 7, 4); // 20:9
    fillsViewport(1024, 768, 7, 4); // 4:3
    fillsViewport(412, 915, 4, 7); // portrait
  });

  test("an item spanning cells includes rectangular cell dimensions and gaps", () => {
    expect(spanSize({ side: 90, cellW: 110, cellH: 80, gap: 8 }, 1, 1)).toEqual({ width: 110, height: 80 });
    expect(spanSize({ side: 90, cellW: 110, cellH: 80, gap: 8 }, 3, 2)).toEqual({ width: 346, height: 168 });
  });

  test("a tiny viewport never gives a zero side", () => {
    expect(gridMetrics({ w: 10, h: 10 }, 8, 5).side).toBeGreaterThanOrEqual(1);
  });

  test("portrait mode transposes 7x4 items to 4x7 in linear order without overlaps", () => {
    const items: LayoutItem[] = [
      { id: "a", kind: "clock", x: 0, y: 0, w: 2, h: 1 },
      { id: "b", kind: "button", x: 2, y: 0, w: 1, h: 1, label: "Play" },
      { id: "c", kind: "button", x: 5, y: 0, w: 1, h: 1, label: "Mute" },
      { id: "d", kind: "button", x: 0, y: 1, w: 1, h: 1, label: "Lock" },
      { id: "e", kind: "weather", x: 4, y: 1, w: 2, h: 2 },
    ];
    const transposed = transposeItemsForGrid(items, 7, 4, 4, 7);
    expect(transposed.map((i) => [i.id, i.x, i.y, i.w, i.h])).toEqual([
      ["a", 0, 0, 2, 1],
      ["b", 2, 0, 1, 1],
      ["c", 3, 0, 1, 1],
      ["d", 0, 1, 1, 1],
      ["e", 1, 1, 2, 2],
    ]);
  });

  test("a full grid with large widgets turns without two tiles on one cell", () => {
    const keys = (xs: number[], y: number): LayoutItem[] => xs.map((x) => ({ id: `k${x}-${y}`, kind: "button", x, y, w: 1, h: 1, label: "K" }));
    // 7x4, every cell taken: three wide widgets and single keys around them.
    const items: LayoutItem[] = [
      ...keys([0], 0),
      { id: "w1", kind: "weather", x: 1, y: 0, w: 3, h: 2 },
      { id: "w2", kind: "media", x: 4, y: 0, w: 3, h: 2 },
      ...keys([0], 1),
      { id: "w3", kind: "clock", x: 0, y: 2, w: 4, h: 2 },
      ...keys([4, 5, 6], 2),
      ...keys([4, 5, 6], 3),
    ];
    const turned = transposeItemsForGrid(items, 7, 4, 4, 7);
    expect(turned.map((i) => i.id).sort()).toEqual(items.map((i) => i.id).sort());
    const cells = turned.flatMap((i) => Array.from({ length: i.w * i.h }, (_, n) => `${i.x + (n % i.w)},${i.y + Math.floor(n / i.w)}`));
    expect(new Set(cells).size).toBe(cells.length);
    for (const i of turned) expect(i.x + i.w <= 4 && i.y + i.h <= 7, i.id).toBe(true);
  });
});
