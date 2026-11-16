import { describe, expect, test } from "vitest";
import { ORIENTATION_KEY, saveOrientation, savedOrientation } from "../src/platform/orientation";

function memory(initial: Record<string, string> = {}) {
  const data = { ...initial };
  return { data, getItem: (k: string) => data[k] ?? null, setItem: (k: string, v: string) => void (data[k] = v) };
}

describe("remembered orientation (DATA_MODEL.md 3.7)", () => {
  test("landscape when nothing was stored or the value is unknown", () => {
    expect(savedOrientation(memory())).toBe("landscape");
    expect(savedOrientation(memory({ [ORIENTATION_KEY]: "diagonal" }))).toBe("landscape");
  });
  test("portrait is read back", () => {
    const store = memory();
    saveOrientation("portrait", store);
    expect(store.data[ORIENTATION_KEY]).toBe("portrait");
    expect(savedOrientation(store)).toBe("portrait");
    saveOrientation("landscape", store);
    expect(savedOrientation(store)).toBe("landscape");
  });
  test("blocked storage never throws", () => {
    const blocked = {
      getItem: () => {
        throw new Error("blocked");
      },
      setItem: () => {
        throw new Error("blocked");
      },
    };
    expect(savedOrientation(blocked)).toBe("landscape");
    expect(() => saveOrientation("portrait", blocked)).not.toThrow();
  });
});
