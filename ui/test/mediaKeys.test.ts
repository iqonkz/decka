import { describe, expect, test } from "vitest";
import { mediaKeySet } from "../src/widgets/mediaKeys";

const ALL = ["prev", "play", "next"];

describe("mediaKeySet (DATA_MODEL.md 6.2)", () => {
  test("h = 1: three keys only when the text block still has 1.5 x side (w >= 5 in practice)", () => {
    expect(mediaKeySet(6, 1, 100, 4)).toEqual(ALL);
    expect(mediaKeySet(5, 1, 240, 8)).toEqual(ALL);
    // w = 4 is allowed three keys by the rule, but they would leave the text ~1.35 x side: prev/next go first.
    expect(mediaKeySet(4, 1, 240, 8)).toEqual(["play"]);
    expect(mediaKeySet(4, 1, 100, 4)).toEqual(["play"]);
  });

  test("h = 1, w = 3 (the owner's widget): not even play fits next to the text, so the text gets the whole width", () => {
    for (const [side, gap] of [[240, 8], [100, 4], [120, 4]] as const) expect(mediaKeySet(3, 1, side, gap)).toEqual([]);
  });
  test("h = 2: keys sit under the text, three from w >= 2 when they fit", () => {
    expect(mediaKeySet(4, 2, 100, 4)).toEqual(ALL);
    expect(mediaKeySet(2, 2, 100, 4)).toEqual([]);
    expect(mediaKeySet(1, 2, 100, 4)).toEqual([]);
  });
});
