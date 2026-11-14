import { describe, expect, test } from "vitest";
import { gridMetrics, spanSize } from "../src/deck/gridMetrics";

const fits = (w: number, h: number, cols: number, rows: number) => {
  const m = gridMetrics({ w, h }, cols, rows);
  const gridW = cols * m.side + (cols - 1) * m.gap;
  const gridH = rows * m.side + (rows - 1) * m.gap;
  expect(m.offsetX + gridW).toBeLessThanOrEqual(w - m.offsetX + 1);
  expect(m.offsetY + gridH).toBeLessThanOrEqual(h - m.offsetY + 1);
  expect(gridW).toBeLessThanOrEqual(w);
  expect(gridH).toBeLessThanOrEqual(h);
  return m;
};

describe("gridMetrics", () => {
  test("915x412 at 7x4: gap is 3-5% of the key, the grid fits", () => {
    const m = fits(915, 412, 7, 4);
    expect(m.gap / m.side).toBeGreaterThanOrEqual(0.03);
    expect(m.gap / m.side).toBeLessThanOrEqual(0.05);
    expect(m.gap).toBeGreaterThanOrEqual(4);
  });

  test("640x360 at 5x3: gap is 3-5% of the key, the grid fits", () => {
    const m = fits(640, 360, 5, 3);
    expect(m.gap / m.side).toBeGreaterThanOrEqual(0.03);
    expect(m.gap / m.side).toBeLessThanOrEqual(0.05);
  });

  test("the grid is centered and keeps the pixel-shift margin", () => {
    const m = gridMetrics({ w: 915, h: 412 }, 7, 4);
    expect(m.offsetX).toBeGreaterThanOrEqual(6);
    expect(m.offsetY).toBeGreaterThanOrEqual(6);
    const gridW = 7 * m.side + 6 * m.gap;
    expect(Math.abs(915 - gridW - 2 * m.offsetX)).toBeLessThanOrEqual(1);
  });

  test("an item spanning cells includes the gaps between them", () => {
    expect(spanSize({ side: 90, gap: 4 }, 1, 1)).toEqual({ width: 90, height: 90 });
    expect(spanSize({ side: 90, gap: 4 }, 3, 2)).toEqual({ width: 278, height: 184 });
  });

  test("a tiny viewport never gives a zero side", () => {
    expect(gridMetrics({ w: 10, h: 10 }, 8, 5).side).toBeGreaterThanOrEqual(1);
  });
});
