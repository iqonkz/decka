import { describe, expect, test } from "vitest";
import { rotateLayout } from "../src/rotate";
import { validateLayout } from "../src/validate";
import type { ActionsFile } from "../src/actions";
import type { LayoutFile, LayoutItem, Page } from "../src/layout";
import layoutDefault from "../fixtures/layout.default.json";
import actionsDefault from "../fixtures/actions.default.json";

const actions = actionsDefault as ActionsFile;
const base = layoutDefault as LayoutFile;
const ok = (r: ReturnType<typeof rotateLayout>) => {
  if (!r.ok) throw new Error(r.error);
  return r;
};
const countItems = (l: LayoutFile) => l.pages.reduce((n, p) => n + p.items.length, 0);
const button = (id: string, x: number, y: number): LayoutItem => ({ id, kind: "button", x, y, w: 1, h: 1, actionId: "lock" });

describe("rotateLayout (DATA_MODEL.md 3.7)", () => {
  /** Two 3x5 widgets on an 8x5 page: in 5x8 the second has no room (3 + 3 > 5 columns, 5 + 5 > 8 rows). */
  const twoTall = (id: string, name: string, extra: Partial<Page> = {}): Page => ({
    id,
    name,
    ...extra,
    items: [{ id: `${id}-a`, kind: "statuses", x: 0, y: 0, w: 3, h: 5 } as LayoutItem, { id: `${id}-b`, kind: "statuses", x: 3, y: 0, w: 3, h: 5 } as LayoutItem],
  });
  const wide: LayoutFile = { ...base, grid: { cols: 8, rows: 5 } };

  test("the default 7x4 becomes 4x7 in reading order: prev, play, next stay in a row, nothing lost, valid", () => {
    expect(base.grid).toEqual({ cols: 7, rows: 4 });
    const r = ok(rotateLayout(base, "portrait"));
    expect(r.layout.grid).toEqual({ cols: 4, rows: 7 });
    expect(r.layout.display.orientation).toBe("portrait");
    expect(r.moved).toBe(0);
    expect(r.layout.pages.map((p) => p.name)).toEqual(["Main", "Dev"]);
    expect(countItems(r.layout)).toBe(countItems(base));
    expect(validateLayout(r.layout, actions)).toEqual([]);

    // Reading order of the new page: widgets first as they were (y, x), then the buttons as they were (y, x).
    const main = r.layout.pages[0]!;
    const reading = (items: LayoutItem[], kind: "button" | "widget") =>
      items.filter((i) => (i.kind === "button") === (kind === "button")).sort((a, b) => a.y - b.y || a.x - b.x).map((i) => i.id);
    expect(reading(main.items, "widget")).toEqual(["w-clock", "w-sys", "w-services", "w-notifs"]);
    const buttons = reading(main.items, "button");
    expect(buttons).toEqual(["b-prev", "b-play", "b-next", "b-mute", "b-voldown", "b-volup", "b-lock", "b-m-tasks", "b-m-notepad", "b-m-github", "b-to-dev"]);
    const at = (id: string) => main.items.find((i) => i.id === id)!;
    // Widgets keep their sizes (the clock stays 2x1).
    expect([at("w-clock").w, at("w-clock").h]).toEqual([2, 1]);
    expect([at("w-notifs").w, at("w-notifs").h]).toEqual([3, 2]);
  });

  test("buttons keep their order of reading when a widget is in the way", () => {
    const l: LayoutFile = {
      ...base,
      grid: { cols: 4, rows: 3 },
      pages: [{ id: "main", name: "Main", items: [{ id: "w", kind: "clock", x: 0, y: 0, w: 2, h: 1 } as LayoutItem, button("prev", 0, 1), button("play", 1, 1), button("next", 2, 1)] }],
    };
    const r = ok(rotateLayout(l, "portrait"));
    expect(r.layout.grid).toEqual({ cols: 3, rows: 4 });
    const at = Object.fromEntries(r.layout.pages[0]!.items.map((i) => [i.id, [i.x, i.y]]));
    // The clock takes (0,0)-(1,0); the buttons follow in reading order: (2,0), (0,1), (1,1).
    expect(at).toEqual({ w: [0, 0], prev: [2, 0], play: [0, 1], next: [1, 1] });
    expect(validateLayout(r.layout, actions)).toEqual([]);
  });

  test("a horizontal volume turns vertical (w and h swap)", () => {
    const vol = { id: "v", kind: "volume", x: 0, y: 0, w: 3, h: 1 } as unknown as LayoutItem;
    const r = ok(rotateLayout({ ...base, pages: [{ id: "main", name: "Main", items: [vol] }] }, "portrait"));
    const got = r.layout.pages[0]!.items[0]!;
    expect([got.w, got.h]).toEqual([1, 3]);
  });

  test("the same orientation changes nothing", () => {
    const r = ok(rotateLayout(base, "landscape"));
    expect(r.layout).toBe(base);
    expect(r.moved).toBe(0);
  });

  test("8x5 with a full page: what does not fit goes to `<name> 2`, nothing is lost and the result is valid", () => {
    const items: LayoutItem[] = [];
    for (let y = 0; y < 5; y++) for (let x = 0; x < 8; x++) items.push(button(`b-${x}-${y}`, x, y));
    const full: LayoutFile = { ...base, grid: { cols: 8, rows: 5 }, pages: [{ id: "main", name: "Main", items }] };
    expect(validateLayout(full, actions)).toEqual([]);
    const r = ok(rotateLayout(full, "portrait"));
    // 5 columns x 8 rows = 40 places for 40 buttons: they all fit, the transposition is exact.
    expect(r.layout.grid).toEqual({ cols: 5, rows: 8 });
    expect(r.moved).toBe(0);

    expect(new Set(r.layout.pages.map((p) => p.id)).size).toBe(r.layout.pages.length);
  });

  test("overflow goes to `<name> 2` without `apps`; the context page keeps its `apps`; nothing is lost", () => {
    const crowded: LayoutFile = { ...wide, pages: [{ id: "main", name: "Main", items: [button("home", 0, 0)] }, twoTall("code", "Code", { apps: ["code.exe"] })] };
    expect(validateLayout(crowded, actions)).toEqual([]);
    const t = ok(rotateLayout(crowded, "portrait"));
    expect(t.layout.grid).toEqual({ cols: 5, rows: 8 });
    expect(t.moved).toBe(1);
    expect(t.layout.pages.map((p) => p.name)).toEqual(["Main", "Code", "Code 2"]);
    expect(t.layout.pages[1]!.apps).toEqual(["code.exe"]);
    expect(t.layout.pages[2]!.apps).toBeUndefined();
    expect(t.layout.pages[2]!.popup).toBeUndefined();
    expect(t.layout.pages[2]!.items.map((i) => i.id)).toEqual(["code-b"]);
    expect(countItems(t.layout)).toBe(countItems(crowded));
    expect(validateLayout(t.layout, actions)).toEqual([]);
  });

  test("a popup group that does not fit is an error with the message of the window, and the layout is not changed", () => {
    const withGroup: LayoutFile = { ...wide, pages: [{ id: "main", name: "Main", items: [button("open", 0, 0)] }, twoTall("grp", "Tools", { popup: true })] };
    const before = JSON.stringify(withGroup);
    const r = rotateLayout(withGroup, "portrait");
    expect(r).toEqual({ ok: false, error: 'Popup group "Tools" doesn\'t fit. Make it smaller first.' });
    expect(JSON.stringify(withGroup)).toBe(before);
  });

  test("the page limit is respected", () => {
    const pages: Page[] = Array.from({ length: 10 }, (_, i) => ({ id: `p${i}`, name: `P${i}`, items: [] }));
    pages[0]!.items = [{ id: "n", kind: "notifications", x: 0, y: 0, w: 8, h: 5 } as LayoutItem];
    expect(rotateLayout({ ...wide, pages }, "portrait").ok).toBe(true); // 8x5 is cut to 5x5 and fits: no overflow
    pages[0] = twoTall("p0", "P0");
    expect(rotateLayout({ ...wide, pages }, "portrait").ok).toBe(false); // the overflow page would be the 11th
  });
});
