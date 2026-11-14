import { describe, expect, it } from "vitest";
import { saverPosition } from "../src/display/saverPosition";

const viewport = { w: 1280, h: 720 };
const box = { w: 300, h: 200 };

describe("saverPosition", () => {
  const seeds = Array.from({ length: 100 }, (_, i) => 58_000_000 + i);

  it("keeps the block inside the screen with a 16 px margin", () => {
    for (const seed of seeds) {
      const { x, y } = saverPosition(seed, viewport, box);
      expect(x).toBeGreaterThanOrEqual(16);
      expect(y).toBeGreaterThanOrEqual(16);
      expect(x + box.w).toBeLessThanOrEqual(viewport.w - 16);
      expect(y + box.h).toBeLessThanOrEqual(viewport.h - 16);
    }
  });

  it("gives at least 20 different positions", () => {
    expect(new Set(seeds.map((s) => JSON.stringify(saverPosition(s, viewport, box)))).size).toBeGreaterThanOrEqual(20);
  });

  it("is deterministic", () => {
    expect(saverPosition(7, viewport, box)).toEqual(saverPosition(7, viewport, box));
  });

  it("falls back to the margin when the block does not fit", () => {
    expect(saverPosition(3, { w: 200, h: 100 }, box)).toEqual({ x: 16, y: 16 });
  });
});
