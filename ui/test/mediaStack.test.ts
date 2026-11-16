import { describe, expect, test } from "vitest";
import { coverSide, isStacked } from "../src/widgets/Media";

describe("the media widget in a narrow, tall place (DESIGN.md 2.2)", () => {
  test("w <= 2 and h >= 2 stacks the cover over the text; everything else stays a row", () => {
    expect(isStacked({ w: 2, h: 2 })).toBe(true);
    expect(isStacked({ w: 2, h: 3 })).toBe(true);
    expect(isStacked({ w: 3, h: 2 })).toBe(false);
    expect(isStacked({ w: 2, h: 1 })).toBe(false);
    expect(isStacked({ w: 4, h: 1 })).toBe(false);
  });
  test("a row's cover is the whole height; a stack's is the width but at most half the height", () => {
    expect(coverSide(false, { width: 400, height: 90 })).toBe(90);
    expect(coverSide(true, { width: 180, height: 400 })).toBe(180);
    expect(coverSide(true, { width: 180, height: 200 })).toBe(100);
    expect(coverSide(true, { width: 0, height: 0 })).toBe(0);
  });
});
