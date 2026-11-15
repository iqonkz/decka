import { describe, expect, test } from "vitest";
import { HAPTICS_KEY, hapticFor, loadHapticsEnabled, saveHapticsEnabled, vibrationFor } from "../src/platform/haptics";

const on = { enabled: true };

describe("hapticFor: one test per row of the DESIGN.md 3a table", () => {
  test("a touch on a key: tap", () => {
    expect(hapticFor({ type: "touch", wokeScreen: false }, on)).toBe("tap");
  });
  test("finishing a confirm hold: confirm", () => {
    expect(hapticFor({ type: "confirm" }, on)).toBe("confirm");
  });
  test("a failed press: error", () => {
    expect(hapticFor({ type: "press_result", ok: false }, on)).toBe("error");
  });
  test("opening a popup group: open", () => {
    expect(hapticFor({ type: "popup_open" }, on)).toBe("open");
  });
  test("a page change by the user: page", () => {
    expect(hapticFor({ type: "page_change", manual: true }, on)).toBe("page");
  });
  test("the long press that opens the phone menu: menu", () => {
    expect(hapticFor({ type: "menu" }, on)).toBe("menu");
  });
});

describe("hapticFor: the exceptions", () => {
  test("a successful press is silent", () => {
    expect(hapticFor({ type: "press_result", ok: true }, on)).toBeNull();
  });
  test("a touch that only wakes the screen does not vibrate", () => {
    expect(hapticFor({ type: "touch", wokeScreen: true }, on)).toBeNull();
  });
  test("a context page change without the user does not vibrate", () => {
    expect(hapticFor({ type: "page_change", manual: false }, on)).toBeNull();
  });
  test("the switch off silences everything", () => {
    const off = { enabled: false };
    for (const event of [
      { type: "touch", wokeScreen: false },
      { type: "confirm" },
      { type: "press_result", ok: false },
      { type: "popup_open" },
      { type: "page_change", manual: true },
      { type: "menu" },
    ] as const) {
      expect(hapticFor(event, off), event.type).toBeNull();
    }
  });
});

describe("browser patterns", () => {
  test("lengths from the table", () => {
    expect(vibrationFor("tap")).toBe(10);
    expect(vibrationFor("confirm")).toBe(20);
    expect(vibrationFor("error")).toEqual([30, 60, 30]);
    expect(vibrationFor("open")).toBe(10);
    expect(vibrationFor("page")).toBe(5);
    expect(vibrationFor("menu")).toBe(20);
  });
});

describe("the phone's switch", () => {
  const fake = () => {
    const data = new Map<string, string>();
    return { getItem: (k: string) => data.get(k) ?? null, setItem: (k: string, v: string) => void data.set(k, v) };
  };
  test("on by default, stored under decka.haptics.v1", () => {
    const s = fake();
    expect(loadHapticsEnabled(s)).toBe(true);
    saveHapticsEnabled(false, s);
    expect(s.getItem(HAPTICS_KEY)).toBe("0");
    expect(loadHapticsEnabled(s)).toBe(false);
    saveHapticsEnabled(true, s);
    expect(loadHapticsEnabled(s)).toBe(true);
  });
  test("blocked storage is tolerated", () => {
    const broken = {
      getItem: () => {
        throw new Error("blocked");
      },
      setItem: () => {
        throw new Error("blocked");
      },
    };
    expect(loadHapticsEnabled(broken)).toBe(true);
    expect(() => saveHapticsEnabled(false, broken)).not.toThrow();
  });
});
