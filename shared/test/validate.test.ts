import { describe, expect, test } from "vitest";
import { validateActions, validateLayout } from "../src/validate";
import type { ActionsFile } from "../src/actions";
import type { LayoutFile } from "../src/layout";
import layoutDefault from "../fixtures/layout.default.json";
import actionsDefault from "../fixtures/actions.default.json";
import overlap from "../fixtures/invalid/layout-overlap.json";
import outOfBounds from "../fixtures/invalid/layout-out-of-bounds.json";
import dupId from "../fixtures/invalid/layout-dup-id.json";
import unknownAction from "../fixtures/invalid/layout-unknown-action.json";
import badNavigate from "../fixtures/invalid/layout-bad-navigate.json";
import badSize from "../fixtures/invalid/layout-bad-size.json";

const actions = actionsDefault as ActionsFile;

describe("validateLayout", () => {
  test("default layout has no issues", () => {
    expect(validateLayout(layoutDefault as LayoutFile, actions)).toEqual([]);
  });

  const cases: [string, { expectPath: string; layout: unknown }][] = [
    ["overlap", overlap],
    ["out-of-bounds", outOfBounds],
    ["dup-id", dupId],
    ["unknown-action", unknownAction],
    ["bad-navigate", badNavigate],
    ["bad-size", badSize],
  ];

  test.each(cases)("%s", (_name, fx) => {
    const issues = validateLayout(fx.layout as LayoutFile, actions);
    expect(issues.length).toBeGreaterThanOrEqual(1);
    expect(issues[0]?.path).toBe(fx.expectPath);
  });
});

describe("validateLayout: configurable grid", () => {
  const base = layoutDefault as LayoutFile;
  const withGrid = (grid: { cols: number; rows: number }, items: unknown[] = []): LayoutFile => ({
    ...base,
    grid,
    pages: [{ id: "main", name: "Main", items: items as LayoutFile["pages"][number]["items"] }],
  });
  const button = (x: number, y: number) => ({ id: "b", kind: "button", x, y, w: 1, h: 1, actionId: "lock" });

  test("8x5 is valid; 9x4 and 7x1 are errors", () => {
    expect(validateLayout(withGrid({ cols: 8, rows: 5 }), actions)).toEqual([]);
    expect(validateLayout(withGrid({ cols: 9, rows: 4 }), actions).map((i) => i.path)).toEqual(["/grid/cols"]);
    expect(validateLayout(withGrid({ cols: 7, rows: 1 }), actions).map((i) => i.path)).toEqual(["/grid/rows"]);
  });

  test("x: 6 fits cols 7 and is an error on cols 6, with the path of the item", () => {
    expect(validateLayout(withGrid({ cols: 7, rows: 4 }, [button(6, 0)]), actions)).toEqual([]);
    const issues = validateLayout(withGrid({ cols: 6, rows: 4 }, [button(6, 0)]), actions);
    expect(issues.map((i) => i.path)).toEqual(["/pages/0/items/0"]);
  });

  test("list widgets may fill the whole grid", () => {
    const widget = { id: "n", kind: "notifications", x: 0, y: 0, w: 8, h: 5 };
    expect(validateLayout(withGrid({ cols: 8, rows: 5 }, [widget]), actions)).toEqual([]);
  });

  test("the default layout with the default actions has no issues and is 7x4", () => {
    expect(base.grid).toEqual({ cols: 7, rows: 4 });
    expect(validateLayout(base, actions)).toEqual([]);
  });
});

describe("validateActions", () => {
  test("default actions have no issues", () => {
    expect(validateActions(actions)).toEqual([]);
  });

  test("duplicate hotkey keys and bad ids are reported", () => {
    const bad = {
      version: 1,
      actions: {
        "Bad Id": { name: "x", type: "system", op: "lock" },
        k: { name: "k", type: "hotkey", keys: ["ctrl", "a", "ctrl"] },
      },
    } as ActionsFile;
    expect(validateActions(bad).map((i) => i.path)).toEqual(["/actions/Bad Id", "/actions/k/keys/2"]);
  });
});
