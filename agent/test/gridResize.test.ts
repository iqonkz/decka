import { describe, expect, test } from "vitest";
import { validateLayout } from "@decka/shared";
import type { LayoutFile, LayoutItem } from "@decka/shared";
import { overflowItems, resizeGridMoving } from "../src/editor/gridResize";

const btn = (id: string, x: number, y: number): LayoutItem => ({ id, kind: "button", x, y, w: 1, h: 1, label: id, actionId: "lock" });
const layout = (items: LayoutItem[]): LayoutFile => ({
  version: 1,
  grid: { cols: 7, rows: 4 },
  display: { dimAfterSec: 60, dimLevel: 0.3, sleepAfterSec: 600, pixelShift: true, night: { enabled: false, from: "23:00", to: "07:00" } },
  pages: [{ id: "main", name: "Main", items }],
});
const actions = { version: 1 as const, actions: { lock: { name: "Lock", type: "system" as const, op: "lock" as const } } };

describe("overflowItems", () => {
  test("lists what sticks out of a smaller grid, with its page", () => {
    const l = layout([btn("a", 0, 0), btn("b", 6, 0), btn("c", 2, 3), { id: "w", kind: "clock", x: 4, y: 0, w: 2, h: 1 }]);
    expect(overflowItems(l, 7, 4)).toEqual([]);
    expect(overflowItems(l, 5, 4).map((o) => o.item.id)).toEqual(["b", "w"]); // the clock at 4..5 crosses the edge too
    expect(overflowItems(l, 5, 3).map((o) => o.item.id)).toEqual(["b", "c", "w"]);
    expect(overflowItems(l, 5, 3)[0]!.page).toBe(0);
  });
  test("a wide widget that crosses the new edge counts", () => {
    const l = layout([{ id: "w", kind: "clock", x: 4, y: 0, w: 2, h: 1 }]);
    expect(overflowItems(l, 5, 4).map((o) => o.item.id)).toEqual(["w"]);
  });
});

describe("resizeGridMoving", () => {
  test("the ones that fit stay; the others go to a new page 'Moved' in free cells; the result validates", () => {
    const l = layout([btn("a", 0, 0), btn("b", 6, 0), btn("c", 6, 1), btn("d", 2, 3)]);
    const r = resizeGridMoving(l, 5, 3);
    expect(r.moved).toBe(3);
    expect(r.lost).toBe(0);
    expect(r.layout.grid).toEqual({ cols: 5, rows: 3 });
    expect(r.layout.pages[0]!.items.map((i) => i.id)).toEqual(["a"]);
    const moved = r.layout.pages[1]!;
    expect(moved.name).toBe("Moved");
    expect(moved.items.map((i) => [i.id, i.x, i.y])).toEqual([["b", 0, 0], ["c", 1, 0], ["d", 2, 0]]);
    expect(validateLayout(r.layout, actions)).toEqual([]);
  });
  test("nothing to move changes only the grid", () => {
    const l = layout([btn("a", 0, 0)]);
    const r = resizeGridMoving(l, 5, 3);
    expect(r.moved).toBe(0);
    expect(r.layout.pages).toHaveLength(1);
  });
  test("many keys fill the new page and continue on another", () => {
    const many = Array.from({ length: 10 }, (_, i) => btn(`k${i}`, 3 + (i % 4), Math.floor(i / 4)));
    // 3x2 = 6 cells: 7..: items at x >= 3 stuck
    const r = resizeGridMoving(layout(many), 3, 2);
    expect(r.moved + r.lost).toBe(10);
    expect(r.layout.pages.slice(1).every((p) => p.items.length <= 6)).toBe(true);
    expect(validateLayout(r.layout, actions)).toEqual([]);
  });
});