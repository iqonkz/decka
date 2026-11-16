import { describe, expect, test } from "vitest";
import type { ButtonItem } from "@decka/shared";
import { keyHasNoData } from "../src/Button";

const key = (v: string): ButtonItem => ({ id: "b", kind: "button", x: 0, y: 0, w: 1, h: 1, actionId: "a", bind: { var: v, states: { true: { color: "#b45309" } } } });

describe("keyHasNoData (DESIGN.md 2.4)", () => {
  test("a reserved variable that does not exist dims the key", () => {
    for (const v of ["audio.muted", "mic.muted", "media.shuffle", "obs.recording"]) expect(keyHasNoData(key(v), {}, false)).toBe(true);
  });
  test("a reserved variable that exists does not", () => {
    expect(keyHasNoData(key("mic.muted"), { "mic.muted": false }, false)).toBe(false);
  });
  test("a user variable that is not set yet is the ordinary state", () => {
    expect(keyHasNoData(key("night"), {}, false)).toBe(false);
  });
  test("the editor preview never dims", () => {
    expect(keyHasNoData(key("obs.recording"), {}, true)).toBe(false);
  });
  test("a key without a binding is never dimmed", () => {
    const { bind: _bind, ...plain } = key("mic.muted");
    expect(keyHasNoData(plain as ButtonItem, {}, false)).toBe(false);
  });
});
