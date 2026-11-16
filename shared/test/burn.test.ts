import { describe, expect, test } from "vitest";
import { burnEta, formatLeft, pushBurnSample, type BurnSample } from "../src/burn";

const MIN = 60_000;
const at = (m: number, pct: number): BurnSample => ({ ts: m * MIN, pct });

describe("burnEta", () => {
  test("growth gives the time the window will be full", () => {
    // 20 points in 20 minutes: one point a minute, 40 left -> 40 more minutes.
    const eta = burnEta([at(0, 10), at(10, 20), at(20, 30)], 20 * MIN, 600 * MIN);
    expect(eta).toBe(20 * MIN + 70 * MIN);
  });

  test("no growth, or less than one point of it, gives no forecast", () => {
    expect(burnEta([at(0, 30), at(10, 30), at(20, 30)], 20 * MIN, 600 * MIN)).toBeNull();
    expect(burnEta([at(0, 30), at(10, 30.4), at(20, 30.8)], 20 * MIN, 600 * MIN)).toBeNull();
  });

  test("too few points, or too short a stretch, gives no forecast", () => {
    expect(burnEta([at(0, 10), at(20, 30)], 20 * MIN, 600 * MIN)).toBeNull();
    expect(burnEta([at(0, 10), at(4, 14), at(8, 18)], 8 * MIN, 600 * MIN)).toBeNull();
    expect(burnEta([], 0, 1)).toBeNull();
  });

  test("a forecast after the reset is no forecast", () => {
    expect(burnEta([at(0, 10), at(10, 20), at(20, 30)], 20 * MIN, 60 * MIN)).toBeNull();
  });
});

describe("pushBurnSample", () => {
  test("a fall of the percentage starts again, and old points leave after an hour", () => {
    const grown = [at(0, 10), at(10, 20)].reduce<BurnSample[]>((list, s) => pushBurnSample(list, s), []);
    expect(grown).toHaveLength(2);
    expect(pushBurnSample(grown, at(11, 2))).toEqual([at(11, 2)]);
    expect(pushBurnSample(grown, at(70, 25)).map((s) => s.ts)).toEqual([10 * MIN, 70 * MIN]);
  });

  test("keeps at most 60 points", () => {
    let list: BurnSample[] = [];
    for (let i = 0; i < 100; i++) list = pushBurnSample(list, { ts: i * 30_000, pct: 10 + i / 100 });
    expect(list.length).toBeLessThanOrEqual(60);
  });
});

describe("formatLeft", () => {
  const u = { d: "d", h: "h", m: "m" };
  test("minutes, hours and days", () => {
    expect(formatLeft(14 * MIN, u)).toBe("14m");
    expect(formatLeft((2 * 60 + 14) * MIN, u)).toBe("2h 14m");
    expect(formatLeft((3 * 1440 + 4 * 60 + 9) * MIN, u)).toBe("3d 4h");
    expect(formatLeft(-5, u)).toBe("0m");
  });
});
