import { describe, expect, test } from "vitest";
import type { DisplaySettings } from "@decka/shared";
import { applyScreenPreset, fracToPercent, minToSec, percentToFrac, screenPreset, secToMin } from "../src/editor/screenPreset";

const base: DisplaySettings = { dimAfterSec: 60, dimLevel: 0.3, sleepAfterSec: 600, pixelShift: true, night: { enabled: false, from: "23:00", to: "07:00" } };

describe("screenPreset", () => {
  test("the defaults are Balanced", () => {
    expect(screenPreset(base)).toBe("balanced");
  });
  test("the other named choices", () => {
    expect(screenPreset({ ...base, dimAfterSec: 30, dimLevel: 0.15, sleepAfterSec: 180 })).toBe("maximum");
    expect(screenPreset({ ...base, dimAfterSec: 0, sleepAfterSec: 0 })).toBe("off");
  });
  test("any difference is Custom", () => {
    expect(screenPreset({ ...base, dimAfterSec: 61 })).toBe("custom");
    expect(screenPreset({ ...base, dimLevel: 0.31 })).toBe("custom");
    expect(screenPreset({ ...base, sleepAfterSec: 0, dimAfterSec: 60 })).toBe("custom");
  });
  test("applying a choice and reading it back agree; other fields are kept", () => {
    for (const p of ["off", "balanced", "maximum"] as const) {
      const d = applyScreenPreset({ ...base, pixelShift: false }, p);
      expect(screenPreset(d)).toBe(p);
      expect(d.pixelShift).toBe(false);
    }
    expect(applyScreenPreset(base, "custom")).toBe(base);
  });
});

describe("units of the Custom fields", () => {
  test("seconds <-> minutes, fraction <-> percent", () => {
    expect(secToMin(60)).toBe(1);
    expect(secToMin(90)).toBe(1.5);
    expect(minToSec(1.5)).toBe(90);
    expect(fracToPercent(0.3)).toBe(30);
    expect(percentToFrac(15)).toBe(0.15);
  });
});