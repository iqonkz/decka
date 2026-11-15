import { describe, expect, test } from "vitest";
import { validateActions, validateLayout } from "@decka/shared";
import type { ActionsFile, InstalledApp, LayoutFile } from "@decka/shared";
import { actionFromApp, actionFromKeys, actionFromUrl, addButtonAt, domainOf, findSameAction, formatChord, makeActionId, presetActions } from "../src/editor/addButton";

const layout = (): LayoutFile => ({
  version: 1,
  grid: { cols: 5, rows: 3 },
  display: { dimAfterSec: 60, dimLevel: 0.3, sleepAfterSec: 600, pixelShift: true, night: { enabled: false, from: "23:00", to: "07:00" } },
  pages: [{ id: "main", name: "Main", items: [{ id: "w-clock", kind: "clock", x: 0, y: 0, w: 2, h: 1 }] }],
});
const emptyActions = (): ActionsFile => ({ version: 1, actions: {} });
const spotify: InstalledApp = { name: "Spotify", program: "C:\\Apps\\Spotify.exe", args: [], icon: "data:image/png;base64,AA" };

const must = (r: ReturnType<typeof addButtonAt>) => {
  if (!r.ok) throw new Error(r.error);
  return r;
};
const valid = (l: LayoutFile, a: ActionsFile) => {
  expect(validateActions(a)).toEqual([]);
  expect(validateLayout(l, a)).toEqual([]);
};

describe("presetActions", () => {
  test("the eight ready-made actions of DESIGN.md 4.5", () => {
    const p = presetActions();
    expect(p.map((x) => x.label)).toEqual(["Play / Pause", "Next track", "Previous track", "Volume up", "Volume down", "Mute", "Lock PC", "Sleep"]);
    expect(p.filter((x) => x.confirm).map((x) => x.id)).toEqual(["lock-pc", "sleep"]);
    expect(p.find((x) => x.id === "volume-up")?.icon).not.toBe(p.find((x) => x.id === "volume-down")?.icon);
  });
});

describe("actionFromUrl / domainOf", () => {
  test("the label is the domain without www.", () => {
    expect(actionFromUrl("https://www.github.com/iqonkz/decka")).toEqual({ name: "github.com", type: "open_url", url: "https://www.github.com/iqonkz/decka" });
    expect(domainOf("https://www.github.com/x")).toBe("github.com");
  });
  test("a bare address gets https://; other schemes and rubbish are refused", () => {
    expect(actionFromUrl("github.com")?.type).toBe("open_url");
    expect(actionFromUrl("github.com")).toMatchObject({ url: "https://github.com/" });
    expect(actionFromUrl("ftp://example.com")).toBeNull();
    expect(actionFromUrl("javascript:alert(1)")).toBeNull();
    expect(actionFromUrl("not a url")).toBeNull();
    expect(actionFromUrl("")).toBeNull();
  });
});

describe("actionFromKeys / formatChord", () => {
  test("a chord is named like Ctrl+Shift+M", () => {
    expect(formatChord(["ctrl", "shift", "m"])).toBe("Ctrl+Shift+M");
    expect(formatChord(["win", "f12"])).toBe("Win+F12");
    expect(formatChord(["alt", "pageup", "left"])).toBe("Alt+PageUp+Left");
    expect(actionFromKeys(["ctrl", "shift", "m"])).toEqual({ name: "Ctrl+Shift+M", type: "hotkey", keys: ["ctrl", "shift", "m"] });
    expect(actionFromKeys([])).toBeNull();
  });
});

describe("makeActionId", () => {
  test("a slug, then -2, -3 when taken", () => {
    expect(makeActionId("Spotify", {})).toBe("spotify");
    expect(makeActionId("Spotify", { spotify: 1 })).toBe("spotify-2");
    expect(makeActionId("Spotify", { spotify: 1, "spotify-2": 1 })).toBe("spotify-3");
  });
  test("only [a-z0-9-], at most 48 characters, never empty", () => {
    expect(makeActionId("Ctrl+Shift+M", {})).toBe("ctrl-shift-m");
    expect(makeActionId("Блокнот", {})).toBe("action");
    const long = makeActionId("x".repeat(100), {});
    expect(long.length).toBeLessThanOrEqual(48);
    expect(makeActionId("x".repeat(100), { [long]: 1 }).length).toBeLessThanOrEqual(48);
    expect(makeActionId("x".repeat(100), { [long]: 1 })).toMatch(/^[a-z0-9-]+$/);
  });
});

describe("findSameAction", () => {
  test("the same content under another name is the same action; different content is not", () => {
    const a: ActionsFile = { version: 1, actions: { old: { name: "Old name", type: "launch", program: "C:\\Apps\\Spotify.exe" } } };
    expect(findSameAction(a, actionFromApp(spotify))).toBe("old");
    expect(findSameAction(a, actionFromApp({ ...spotify, args: ["--x"] }))).toBeUndefined();
    expect(findSameAction(a, { name: "Spotify", type: "open_url", url: "https://a.com/" })).toBeUndefined();
  });
});

describe("addButtonAt", () => {
  test("a program: a button with its name and icon, a launch action; the result validates", () => {
    const r = must(addButtonAt(layout(), emptyActions(), 0, { x: 2, y: 0 }, { type: "app", app: spotify, icon: "img:0123456789abcdef" }));
    const item = r.layout.pages[0]!.items.find((i) => i.id === r.itemId);
    expect(item).toMatchObject({ kind: "button", x: 2, y: 0, label: "Spotify", icon: "img:0123456789abcdef", actionId: "spotify" });
    expect(r.actions.actions.spotify).toEqual({ name: "Spotify", type: "launch", program: "C:\\Apps\\Spotify.exe" });
    expect(r.actionsChanged).toBe(true);
    valid(r.layout, r.actions);
  });

  test("a program without an icon gets the rocket", () => {
    const r = must(addButtonAt(layout(), emptyActions(), 0, { x: 2, y: 0 }, { type: "app", app: spotify }));
    expect(r.layout.pages[0]!.items.at(-1)).toMatchObject({ icon: "lucide:rocket" });
  });

  test("the same program twice reuses the action", () => {
    const first = must(addButtonAt(layout(), emptyActions(), 0, { x: 2, y: 0 }, { type: "app", app: spotify }));
    const second = must(addButtonAt(first.layout, first.actions, 0, { x: 3, y: 0 }, { type: "app", app: spotify }));
    expect(Object.keys(second.actions.actions)).toEqual(["spotify"]);
    expect(second.actionsChanged).toBe(false);
    expect(second.layout.pages[0]!.items.filter((i) => i.kind === "button").map((i) => (i as { actionId?: string }).actionId)).toEqual(["spotify", "spotify"]);
    valid(second.layout, second.actions);
  });

  test("two different programs with the same name get id suffixes", () => {
    const first = must(addButtonAt(layout(), emptyActions(), 0, { x: 2, y: 0 }, { type: "app", app: spotify }));
    const other: InstalledApp = { ...spotify, program: "D:\\Other\\Spotify.exe" };
    const second = must(addButtonAt(first.layout, first.actions, 0, { x: 3, y: 0 }, { type: "app", app: other }));
    expect(Object.keys(second.actions.actions).sort()).toEqual(["spotify", "spotify-2"]);
    valid(second.layout, second.actions);
  });

  test("the label is cut to 24 characters", () => {
    const longName = "A very long program name that does not fit";
    const r = must(addButtonAt(layout(), emptyActions(), 0, { x: 2, y: 0 }, { type: "app", app: { ...spotify, name: longName } }));
    expect(r.layout.pages[0]!.items.at(-1)).toMatchObject({ label: longName.slice(0, 24) });
    valid(r.layout, r.actions);
  });

  test("ready-made actions: Lock PC asks before running, volume buttons differ in icon", () => {
    const lock = must(addButtonAt(layout(), emptyActions(), 0, { x: 2, y: 0 }, { type: "preset", presetId: "lock-pc" }));
    expect(lock.layout.pages[0]!.items.at(-1)).toMatchObject({ label: "Lock PC", icon: "lucide:lock", confirm: true });
    expect(lock.actions.actions["lock-pc"]).toEqual({ name: "Lock PC", type: "system", op: "lock" });
    const up = must(addButtonAt(lock.layout, lock.actions, 0, { x: 3, y: 0 }, { type: "preset", presetId: "volume-up" }));
    valid(up.layout, up.actions);
  });

  test("a website and a shortcut", () => {
    const web = must(addButtonAt(layout(), emptyActions(), 0, { x: 2, y: 0 }, { type: "url", url: "https://www.github.com/" }));
    expect(web.layout.pages[0]!.items.at(-1)).toMatchObject({ label: "github.com", icon: "lucide:globe" });
    const keys = must(addButtonAt(web.layout, web.actions, 0, { x: 3, y: 0 }, { type: "keys", keys: ["ctrl", "shift", "m"] }));
    expect(keys.layout.pages[0]!.items.at(-1)).toMatchObject({ label: "Ctrl+Shift+M", icon: "lucide:zap" });
    valid(keys.layout, keys.actions);
    expect(addButtonAt(layout(), emptyActions(), 0, { x: 2, y: 0 }, { type: "url", url: "nope" })).toMatchObject({ ok: false });
  });

  test("an existing action keeps its name and gets an icon by type", () => {
    const actions: ActionsFile = { version: 1, actions: { "task-manager": { name: "Task Manager", type: "hotkey", keys: ["ctrl", "shift", "esc"] } } };
    const r = must(addButtonAt(layout(), actions, 0, { x: 2, y: 0 }, { type: "action", actionId: "task-manager" }));
    expect(r.layout.pages[0]!.items.at(-1)).toMatchObject({ label: "Task Manager", icon: "lucide:zap", actionId: "task-manager" });
    expect(r.actionsChanged).toBe(false);
    valid(r.layout, r.actions);
  });

  test("a widget needs room for its size; media gets the actions it needs", () => {
    const w = must(addButtonAt(layout(), emptyActions(), 0, { x: 2, y: 1 }, { type: "widget", kind: "sys" }));
    expect(w.layout.pages[0]!.items.at(-1)).toMatchObject({ kind: "sys", x: 2, y: 1 });
    expect(addButtonAt(layout(), emptyActions(), 0, { x: 4, y: 1 }, { type: "widget", kind: "sys" })).toMatchObject({ ok: false });
  });

  test("a taken cell, a missing page or an unknown choice is an error and changes nothing", () => {
    expect(addButtonAt(layout(), emptyActions(), 0, { x: 0, y: 0 }, { type: "app", app: spotify })).toMatchObject({ ok: false });
    expect(addButtonAt(layout(), emptyActions(), 3, { x: 2, y: 0 }, { type: "app", app: spotify })).toMatchObject({ ok: false });
    expect(addButtonAt(layout(), emptyActions(), 0, { x: 2, y: 0 }, { type: "preset", presetId: "nope" })).toMatchObject({ ok: false });
    expect(addButtonAt(layout(), emptyActions(), 0, { x: 2, y: 0 }, { type: "action", actionId: "nope" })).toMatchObject({ ok: false });
  });
});
