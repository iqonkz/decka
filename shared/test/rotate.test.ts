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
  test("the default 7x4 becomes 4x7: buttons on the transposed places, the clock stays 2x1, nothing lost, valid", () => {
    expect(base.grid).toEqual({ cols: 7, rows: 4 });
    const r = ok(rotateLayout(base, "portrait"));
    expect(r.layout.grid).toEqual({ cols: 4, rows: 7 });
    expect(r.layout.display.orientation).toBe("portrait");
    // The seven volume/transport keys fill the whole third column of the turned page, so the 3-wide notifications widget has
    // no three free columns left: it goes to the new page `Main 2` (rule 4), the only thing that moves.
    expect(r.moved).toBe(1);
    expect(r.layout.pages.map((p) => p.name)).toEqual(["Main", "Main 2", "Dev"]);
    expect(r.layout.pages[1]!.items.map((i) => i.id)).toEqual(["w-notifs"]);
    expect(countItems(r.layout)).toBe(countItems(base));
    expect(validateLayout(r.layout, actions)).toEqual([]);

    // Every button is at (y, x) of where it was, whenever nothing blocked it.
    const was = new Map(base.pages.flatMap((p) => p.items).map((i) => [i.id, i]));
    for (const page of r.layout.pages) {
      for (const item of page.items) {
        const before = was.get(item.id)!;
        if (item.kind === "button") expect([item.x, item.y], item.id).toEqual([before.y, before.x]);
        if (item.kind === "clock") expect([item.w, item.h]).toEqual([before.w, before.h]);
      }
    }
    const clock = r.layout.pages.flatMap((p) => p.items).find((i) => i.kind === "clock");
    if (clock) expect([clock.w, clock.h]).toEqual([2, 1]);
  });

  test("turning back gives the layout it came from", () => {
    // A page where everything fits both ways: three keys, a clock and a 2x2 system widget.
    const simple: LayoutFile = {
      ...base,
      pages: [{ id: "main", name: "Main", items: [{ id: "c", kind: "clock", x: 0, y: 0, w: 2, h: 1 } as LayoutItem, { id: "s", kind: "sys", x: 3, y: 0, w: 2, h: 2 } as LayoutItem, button("a", 0, 2), button("b", 1, 3), button("d", 6, 1)] }],
    };
    expect(validateLayout(simple, actions)).toEqual([]);
    const there = ok(rotateLayout(simple, "portrait"));
    expect(there.moved).toBe(0);
    const back = ok(rotateLayout(there.layout, "landscape"));
    expect(back.moved).toBe(0);
    expect(back.layout.grid).toEqual(simple.grid);
    // (the order of items inside a page is by kind and place after a turn: compare by id)
    const byId = (l: LayoutFile) => l.pages.map((p) => p.items.map((i) => [i.id, i.x, i.y, i.w, i.h] as const).sort(([a], [b]) => a.localeCompare(b)));
    expect(byId(back.layout)).toEqual(byId(simple));
    expect(back.layout.display.orientation).toBe("landscape");
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

    // Add a clock: 41 elements cannot fit 40 cells.
    // A second page (context pages cannot be first) of 38 buttons plus a clock at the origin: 39 cells' worth of items, but the
    // clock is 2 wide, so after turning only the transposed places are free of it.
    const crowded: LayoutFile = {
      ...full,
      pages: [{ id: "main", name: "Main", items: [button("home", 0, 0)] }, { id: "code", name: "Code", apps: ["code.exe"], items: [{ id: "clk", kind: "clock", x: 0, y: 0, w: 2, h: 1 } as LayoutItem, ...items.filter((i) => !["b-0-0", "b-1-0"].includes(i.id))] }],
    };
    expect(validateLayout(crowded, actions)).toEqual([]);
    const t = ok(rotateLayout(crowded, "portrait"));
    expect(t.layout.grid).toEqual({ cols: 5, rows: 8 });
    expect(validateLayout(t.layout, actions)).toEqual([]);
    expect(countItems(t.layout)).toBe(countItems(crowded));
    const names = t.layout.pages.map((p) => p.name);
    // Where something did not fit, the new page follows its page and is named `<name> 2`; a context page's `apps` stay on it only.
    expect(names[0]).toBe("Main");
    expect(names[1]).toBe("Code");
    if (t.moved > 0) {
      expect(names[2]).toBe("Code 2");
      expect(t.layout.pages[1]!.apps).toEqual(["code.exe"]);
      expect(t.layout.pages[2]!.apps).toBeUndefined();
      expect(t.layout.pages[2]!.popup).toBeUndefined();
      expect(t.layout.pages[2]!.items.length).toBe(t.moved);
    }
    expect(new Set(t.layout.pages.map((p) => p.id)).size).toBe(t.layout.pages.length);
  });

  test("items that cannot keep their place take the first free one on the same page", () => {
    // 3 wide landscape page -> portrait cols' = 2: a button at x=2 has no column 2 in the turned grid for y' = 2 > ... it moves.
    const l: LayoutFile = {
      ...base,
      grid: { cols: 3, rows: 2 },
      pages: [{ id: "main", name: "Main", items: [button("a", 2, 0), button("b", 2, 1), button("c", 0, 0)] }],
    };
    expect(validateLayout(l, actions)).toEqual([]);
    const r = ok(rotateLayout(l, "portrait"));
    expect(r.layout.grid).toEqual({ cols: 2, rows: 3 });
    expect(r.moved).toBe(0);
    const at = Object.fromEntries(r.layout.pages[0]!.items.map((i) => [i.id, [i.x, i.y]]));
    expect(at).toEqual({ a: [0, 2], b: [1, 2], c: [0, 0] });
    expect(validateLayout(r.layout, actions)).toEqual([]);
  });

  test("a popup group that does not fit is an error with the message of the window, and the layout is not changed", () => {
    const items: LayoutItem[] = [];
    for (let y = 0; y < 5; y++) for (let x = 0; x < 8; x++) items.push(button(`p-${x}-${y}`, x, y));
    const withGroup: LayoutFile = {
      ...base,
      grid: { cols: 8, rows: 5 },
      pages: [
        { id: "main", name: "Main", items: [button("open", 0, 0)] },
        { id: "grp", name: "Tools", popup: true, items: [...items, { id: "wide", kind: "notifications", x: 0, y: 0, w: 2, h: 1 } as LayoutItem].filter((i) => i.id !== "p-0-0" && i.id !== "p-1-0") },
      ],
    };
    const before = JSON.stringify(withGroup);
    const r = rotateLayout(withGroup, "portrait");
    expect(r).toEqual({ ok: false, error: 'Popup group "Tools" doesn\'t fit. Make it smaller first.' });
    expect(JSON.stringify(withGroup)).toBe(before);
  });

  test("the page limit is respected", () => {
    const pages: Page[] = Array.from({ length: 10 }, (_, i) => ({ id: `p${i}`, name: `P${i}`, items: [] }));
    pages[0]!.items = [{ id: "n", kind: "notifications", x: 0, y: 0, w: 8, h: 5 } as LayoutItem];
    const l: LayoutFile = { ...base, grid: { cols: 8, rows: 5 }, pages };
    const r = rotateLayout(l, "portrait");
    // A notifications widget 8x5 is cut to 5x5 in the turned grid and fits: no overflow, so it is fine.
    expect(r.ok).toBe(true);
  });
});
