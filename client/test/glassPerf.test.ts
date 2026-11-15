import { describe, expect, test } from "vitest";
import { averageFps, loadReduced, REDUCED_KEY, saveReduced, shouldReduce } from "../src/platform/glassPerf";

describe("averageFps", () => {
  test("60 frames in one second", () => {
    const times = Array.from({ length: 61 }, (_, i) => i * (1000 / 60));
    expect(averageFps(times)).toBeCloseTo(60, 5);
  });
  test("too few frames is unknown", () => {
    expect(averageFps([])).toBeNull();
    expect(averageFps([5])).toBeNull();
    expect(averageFps([5, 5])).toBeNull();
  });
});

describe("shouldReduce", () => {
  test("below 45 fps reduces, 45 and above does not, unknown does not", () => {
    expect(shouldReduce(44.9)).toBe(true);
    expect(shouldReduce(45)).toBe(false);
    expect(shouldReduce(120)).toBe(false);
    expect(shouldReduce(null)).toBe(false);
  });
});

describe("stored decision", () => {
  const fake = () => {
    const data = new Map<string, string>();
    return { getItem: (k: string) => data.get(k) ?? null, setItem: (k: string, v: string) => void data.set(k, v) };
  };

  test("round trip and the key", () => {
    const s = fake();
    expect(loadReduced(s)).toBeNull();
    saveReduced(true, s);
    expect(s.getItem(REDUCED_KEY)).toBe("1");
    expect(loadReduced(s)).toBe(true);
    saveReduced(false, s);
    expect(loadReduced(s)).toBe(false);
  });

  test("storage that throws is tolerated", () => {
    const broken = {
      getItem: () => {
        throw new Error("blocked");
      },
      setItem: () => {
        throw new Error("blocked");
      },
    };
    expect(loadReduced(broken)).toBeNull();
    expect(() => saveReduced(true, broken)).not.toThrow();
  });
});
