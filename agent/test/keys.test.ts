import { describe, expect, test } from "vitest";
import { formatKeys, isKeyName, parseKeys } from "../src/tabs/keys";

describe("parseKeys", () => {
  test("is case and space tolerant", () => {
    expect(parseKeys("Ctrl + Shift + M")).toEqual(["ctrl", "shift", "m"]);
    expect(parseKeys("ctrl+shift+esc")).toEqual(["ctrl", "shift", "esc"]);
    expect(parseKeys("F24")).toEqual(["f24"]);
  });

  test("rejects repeats, too many keys, and unknown names", () => {
    expect(parseKeys("ctrl+ctrl")).toBeNull();
    expect(parseKeys("ctrl+c+x+v+b+n")).toBeNull();
    expect(parseKeys("hyper+a")).toBeNull();
  });

  test("rejects empty parts and the empty string", () => {
    expect(parseKeys("")).toBeNull();
    expect(parseKeys("ctrl+")).toBeNull();
    expect(parseKeys("+a")).toBeNull();
  });

  test("accepts exactly five keys", () => {
    expect(parseKeys("ctrl+alt+shift+win+a")).toHaveLength(5);
  });
});

describe("isKeyName", () => {
  test("covers the documented set and nothing else", () => {
    for (const k of ["ctrl", "alt", "shift", "win", "a", "z", "0", "9", "f1", "f24", "enter", "pagedown", "right"]) expect(isKeyName(k), k).toBe(true);
    for (const k of ["f25", "f0", "A", "ab", "ctrl+c", "capslock", ""]) expect(isKeyName(k), k).toBe(false);
  });
});

test("formatKeys joins with +", () => {
  expect(formatKeys(["ctrl", "shift", "m"])).toBe("ctrl+shift+m");
});
