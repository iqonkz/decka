import { describe, expect, test } from "vitest";
import { TABLES, type Language } from "@decka/shared";
import { aiMode, closestWindow, formatReset, levelOf, MIN_TEXT, rowsFit, textWidth, windowLabel } from "../src/widgets/aiModel";

const units = (lang: Language) => ({ d: TABLES[lang]["ai.unit_d"] as string, h: TABLES[lang]["ai.unit_h"] as string, m: TABLES[lang]["ai.unit_m"] as string });
const MIN = 60_000;

describe("AI widget: the time to a reset (DESIGN.md 4.11)", () => {
  test("2h 14m, 3d, 14m in English", () => {
    const u = units("en");
    expect(formatReset(134 * MIN, u)).toBe("2h 14m");
    expect(formatReset(3 * 24 * 60 * MIN + 5 * 60 * MIN, u)).toBe("3d");
    expect(formatReset(14 * MIN, u)).toBe("14m");
  });
  test("Russian and Kazakh use their own unit letters", () => {
    expect(formatReset(134 * MIN, units("ru"))).toBe("2ч 14м");
    expect(formatReset(3 * 24 * 60 * MIN, units("ru"))).toBe("3д");
    expect(formatReset(14 * MIN, units("ru"))).toBe("14м");
    expect(formatReset(134 * MIN, units("kk"))).toBe("2сағ 14мин");
    expect(formatReset(3 * 24 * 60 * MIN, units("kk"))).toBe("3күн");
    expect(formatReset(14 * MIN, units("kk"))).toBe("14мин");
  });
  test("a window label takes the translated unit", () => {
    expect(windowLabel("5h", units("ru"))).toBe("5ч");
    expect(windowLabel("7d", units("ru"))).toBe("7д");
    expect(windowLabel("30d", units("en"))).toBe("30d");
    expect(windowLabel("weird", units("en"))).toBe("weird");
  });
  test("a negative or tiny time is 0m, never negative", () => {
    expect(formatReset(-5 * MIN, units("en"))).toBe("0m");
  });
});

describe("AI widget: level and window", () => {
  test("the level changes at 70 and 90", () => {
    expect(levelOf(0)).toBe("accent");
    expect(levelOf(69.9)).toBe("accent");
    expect(levelOf(70)).toBe("warning");
    expect(levelOf(89.9)).toBe("warning");
    expect(levelOf(90)).toBe("error");
    expect(levelOf(100)).toBe("error");
  });
  test("the window closest to its limit wins; a tie goes to the shorter window", () => {
    const a = { label: "5h", pct: 40, resetsAt: 1 };
    const b = { label: "7d", pct: 82, resetsAt: 2 };
    expect(closestWindow([a, b])).toBe(b);
    expect(closestWindow([b, a])).toBe(b);
    expect(closestWindow([a, { ...b, pct: 40 }])).toBe(a);
    expect(closestWindow([])).toBeNull();
  });
});

describe("AI widget: modes and what fits", () => {
  test("2x1 rings, 2x2 concentric rings, from 3x2 rows", () => {
    expect(aiMode(2, 1)).toBe("rings1");
    expect(aiMode(4, 1)).toBe("rings1");
    expect(aiMode(2, 2)).toBe("rings2");
    expect(aiMode(3, 2)).toBe("rows");
    expect(aiMode(4, 2)).toBe("rows");
  });
  test("labels go first, then the reset line; nothing is cut", () => {
    expect(rowsFit(260, 80, true, "↻ 2h 14m")).toEqual({ labels: true, reset: true, bars: 2 });
    expect(rowsFit(100, 80, true, "↻ 2h 14m").labels).toBe(false);
    expect(rowsFit(260, 30, true, "↻ 2h 14m").reset).toBe(false);
    expect(rowsFit(30, 80, true, "↻ 2h 14m").reset).toBe(false);
  });
  test("the text is never smaller than 11 px", () => {
    expect(MIN_TEXT).toBe(11);
    expect(textWidth("100%", 15)).toBeGreaterThan(textWidth("62%", 15));
  });
});
