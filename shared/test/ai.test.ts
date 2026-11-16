import { describe, expect, test } from "vitest";
import { fitWidgetSize, isReservedVar, sizeAllowed } from "../src";

describe("the ai widget (DATA_MODEL.md 6.8)", () => {
  const grid = { cols: 7, rows: 4 };

  test("sizes are 2x1, 2x2, 3x2 and 4x2", () => {
    for (const [w, h, ok] of [[2, 1, true], [2, 2, true], [3, 2, true], [4, 2, true], [3, 1, false], [4, 1, false], [1, 1, false], [5, 2, false], [2, 3, false]] as const) {
      expect(sizeAllowed("ai", w, h, grid), `${w}x${h}`).toBe(ok);
    }
  });

  test("turning the deck fits it to a size that exists", () => {
    expect(fitWidgetSize("ai", 4, 2, 4, 1)).toEqual({ w: 2, h: 1 });
    expect(fitWidgetSize("ai", 4, 2, 3, 2)).toEqual({ w: 3, h: 2 });
    expect(fitWidgetSize("ai", 4, 2, 2, 8)).toEqual({ w: 2, h: 2 });
  });

  test("ai. variables belong to Decka", () => {
    expect(isReservedVar("ai.claude.5h")).toBe(true);
    expect(isReservedVar("ai")).toBe(false);
    expect(isReservedVar("aid.x")).toBe(false);
  });
});
