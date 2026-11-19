import { describe, expect, test } from "vitest";
import { chordToState, keyLabel, searchKeys, stateToChord } from "../src/editor/chord";

describe("chordToState", () => {
  test("splits modifiers from the main key", () => {
    expect(chordToState(["win", "shift", "s"])).toEqual({ ctrl: false, alt: false, shift: true, win: true, key: "s" });
  });
  test("a chord of modifiers only has no key", () => {
    expect(chordToState(["ctrl", "alt"]).key).toBeNull();
  });
});

describe("stateToChord", () => {
  const none = { ctrl: false, alt: false, shift: false, win: false, key: null };
  test("modifiers come in a fixed order, then the key", () => {
    expect(stateToChord({ ...none, win: true, ctrl: true, shift: true, key: "s" })).toEqual({ ok: true, keys: ["ctrl", "shift", "win", "s"] });
  });
  test("a key alone is a chord of one", () => {
    expect(stateToChord({ ...none, key: "f5" })).toEqual({ ok: true, keys: ["f5"] });
  });
  test("a lone win is allowed (it opens Start)", () => {
    expect(stateToChord({ ...none, win: true })).toEqual({ ok: true, keys: ["win"] });
  });
  test("without a key it is an error, also for other modifiers", () => {
    expect(stateToChord(none)).toEqual({ ok: false, error: "pick_key" });
    expect(stateToChord({ ...none, ctrl: true })).toEqual({ ok: false, error: "pick_key" });
    expect(stateToChord({ ...none, win: true, alt: true })).toEqual({ ok: false, error: "pick_key" });
  });
  test("state and chord round-trip", () => {
    for (const keys of [["ctrl", "alt", "delete"], ["alt", "tab"], ["win", "d"], ["win"], ["printscreen"]]) {
      expect(stateToChord(chordToState(keys))).toEqual({ ok: true, keys });
    }
  });
});

describe("key search", () => {
  test("finds by caption and by name, and labels are readable", () => {
    expect(searchKeys("print").flatMap((g) => g.keys)).toEqual(["printscreen"]);
    expect(searchKeys("page").flatMap((g) => g.keys)).toEqual(["pageup", "pagedown"]);
    expect(searchKeys("nothing-like-this")).toEqual([]);
    expect(keyLabel("pageup")).toBe("Page Up");
    expect(keyLabel("period")).toBe(".");
    expect(keyLabel("f12")).toBe("F12");
  });
});
