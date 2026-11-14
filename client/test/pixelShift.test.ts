import { describe, expect, it } from "vitest";
import { shiftOffset } from "../src/display/pixelShift";

describe("shiftOffset", () => {
  const steps = Array.from({ length: 16 }, (_, i) => shiftOffset(i));

  it("stays within 4 px and reaches 4", () => {
    for (const { x, y } of steps) {
      expect(Math.abs(x)).toBeLessThanOrEqual(4);
      expect(Math.abs(y)).toBeLessThanOrEqual(4);
    }
    expect(Math.max(...steps.map((s) => Math.abs(s.x)))).toBe(4);
    expect(Math.max(...steps.map((s) => Math.abs(s.y)))).toBe(4);
  });

  it("starts at the centre and repeats every 16 steps", () => {
    expect(shiftOffset(0)).toEqual({ x: 0, y: 0 });
    expect(shiftOffset(16)).toEqual(shiftOffset(0));
    expect(shiftOffset(37)).toEqual(shiftOffset(5));
  });

  it("visits at least 8 different offsets", () => {
    expect(new Set(steps.map((s) => `${s.x},${s.y}`)).size).toBeGreaterThanOrEqual(8);
  });
});
