import { describe, expect, test } from "vitest";
import type { ButtonItem } from "@decka/shared";
import { pressErrorTitle, resolveLook } from "../src/look";

const mute: ButtonItem = {
  id: "b-mute",
  kind: "button",
  x: 0,
  y: 2,
  w: 1,
  h: 1,
  icon: "lucide:volume-2",
  actionId: "vol-mute",
  bind: { var: "audio.muted", states: { true: { icon: "lucide:volume-x", color: "#b45309" } } },
};

describe("resolveLook", () => {
  test("no bind gives the base look", () => {
    const { bind: _bind, ...plain } = mute;
    expect(resolveLook(plain, { "audio.muted": true })).toEqual({ icon: "lucide:volume-2" });
  });

  test("a true variable overlays only the fields the state defines", () => {
    const item: ButtonItem = { ...mute, label: "Mute" };
    expect(resolveLook(item, { "audio.muted": true })).toEqual({ label: "Mute", icon: "lucide:volume-x", color: "#b45309" });
  });

  test("a missing variable uses the 'false' state when present", () => {
    const item: ButtonItem = { ...mute, bind: { var: "audio.muted", states: { false: { color: "#111111" } } } };
    expect(resolveLook(item, {})).toEqual({ icon: "lucide:volume-2", color: "#111111" });
  });

  test("a value without a matching state gives the base look", () => {
    expect(resolveLook(mute, { "audio.muted": "x" })).toEqual({ icon: "lucide:volume-2" });
    expect(resolveLook(mute, {})).toEqual({ icon: "lucide:volume-2" }); // no 'false' key either
  });
});

describe("pressErrorTitle", () => {
  test("maps known codes and falls back to the message", () => {
    expect(pressErrorTitle({ code: "UNKNOWN_ACTION" })).toBe("Action is not defined on the PC");
    expect(pressErrorTitle({ code: "ACTION_FAILED" })).toBe("Action failed on the PC");
    expect(pressErrorTitle({ code: "RATE_LIMITED" })).toBe("Too fast");
    expect(pressErrorTitle({ code: "TIMEOUT" })).toBe("PC did not respond");
    expect(pressErrorTitle({ code: "OFFLINE" })).toBe("Not connected");
    expect(pressErrorTitle({ code: "WEIRD", message: "something" })).toBe("something");
  });
});
