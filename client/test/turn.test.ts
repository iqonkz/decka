import { describe, expect, test } from "vitest";
import type { StripInfo } from "@decka/shared";
import { loadTurn, orientationLock, saveTurn, TURN_KEY, turnRoute, turnSide } from "../src/platform/turn";
import { hiddenStripApps, loadStripModes, stripModeFor, stripPage, STRIP_KEY } from "../src/deck/stripMode";

function memory(initial: Record<string, string> = {}) {
  const data = { ...initial };
  return { data, getItem: (k: string) => data[k] ?? null, setItem: (k: string, v: string) => void (data[k] = v) };
}

describe("orientationLock (DATA_MODEL.md 10.9)", () => {
  test("the six combinations", () => {
    expect(orientationLock("landscape", "auto")).toBe("landscape");
    expect(orientationLock("landscape", "normal")).toBe("landscape-primary");
    expect(orientationLock("landscape", "flipped")).toBe("landscape-secondary");
    expect(orientationLock("portrait", "auto")).toBe("portrait");
    expect(orientationLock("portrait", "normal")).toBe("portrait-primary");
    expect(orientationLock("portrait", "flipped")).toBe("portrait-secondary");
  });
  test("auto goes to the sensor plugin, the others to the screen-orientation plugin", () => {
    expect(turnRoute("landscape", "auto")).toEqual({ via: "sensor", portrait: false });
    expect(turnRoute("portrait", "auto")).toEqual({ via: "sensor", portrait: true });
    expect(turnRoute("landscape", "normal")).toEqual({ via: "plugin", lock: "landscape-primary" });
    expect(turnRoute("landscape", "flipped")).toEqual({ via: "plugin", lock: "landscape-secondary" });
    expect(turnRoute("portrait", "normal")).toEqual({ via: "plugin", lock: "portrait-primary" });
    expect(turnRoute("portrait", "flipped")).toEqual({ via: "plugin", lock: "portrait-secondary" });
  });
  test("turnSide sectors, landscape and portrait (T20.1)", () => {
    const angles = [0, 45, 90, 135, 200, 270, 330];
    // landscape: 60..120 secondary, 240..300 primary, the rest keeps the current side
    expect(angles.map((a) => turnSide(a, "primary", false))).toEqual(["primary", "primary", "secondary", "primary", "primary", "primary", "primary"]);
    expect(angles.map((a) => turnSide(a, "secondary", false))).toEqual(["secondary", "secondary", "secondary", "secondary", "secondary", "primary", "secondary"]);
    // portrait: 330..30 primary, 150..210 secondary
    expect(angles.map((a) => turnSide(a, "primary", true))).toEqual(["primary", "primary", "primary", "primary", "secondary", "primary", "primary"]);
    expect(angles.map((a) => turnSide(a, "secondary", true))).toEqual(["primary", "secondary", "secondary", "secondary", "secondary", "secondary", "primary"]);
    expect(turnSide(-1, "secondary", false)).toBe("secondary");
    expect(turnSide(Number.NaN, "primary", true)).toBe("primary");
  });
  test("an absent orientation is landscape", () => {
    expect(orientationLock(undefined, "flipped")).toBe("landscape-secondary");
  });
  test("the turn is remembered; unknown values and blocked storage give auto", () => {
    const store = memory();
    expect(loadTurn(store)).toBe("auto");
    saveTurn("flipped", store);
    expect(store.data[TURN_KEY]).toBe("flipped");
    expect(loadTurn(store)).toBe("flipped");
    expect(loadTurn(memory({ [TURN_KEY]: "sideways" }))).toBe("auto");
    expect(loadTurn({ getItem: () => { throw new Error("blocked"); } })).toBe("auto");
  });
});

describe("strip modes (DATA_MODEL.md 10.5)", () => {
  test("auto by default, stored values are read back, junk is dropped", () => {
    const modes = loadStripModes(memory({ [STRIP_KEY]: JSON.stringify({ "winword.exe": "never", "excel.exe": "tab", "x.exe": "loud" }) }));
    expect(stripModeFor(modes, "winword.exe")).toBe("never");
    expect(stripModeFor(modes, "excel.exe")).toBe("tab");
    expect(stripModeFor(modes, "x.exe")).toBe("auto");
    expect(stripModeFor(modes, "other.exe")).toBe("auto");
    expect(hiddenStripApps(modes)).toEqual(["winword.exe"]);
    expect(loadStripModes(memory({ [STRIP_KEY]: "not json" }))).toEqual({});
  });
});

describe("stripPage", () => {
  const strip: StripInfo = { app: "winword.exe", template: "word", name: "Word", keys: [1, 2, 3, 4, 5].map((n) => ({ actionId: `tpl-word-${n}`, label: `K${n}` })) };
  test("two columns in landscape, two rows in portrait, in reading order", () => {
    const land = stripPage(strip, false).items.map((i) => [(i as { x: number }).x, (i as { y: number }).y]);
    expect(land).toEqual([[0, 0], [1, 0], [0, 1], [1, 1], [0, 2]]);
    const port = stripPage(strip, true).items.map((i) => [(i as { x: number }).x, (i as { y: number }).y]);
    expect(port).toEqual([[0, 0], [0, 1], [1, 0], [1, 1], [2, 0]]);
  });
});
