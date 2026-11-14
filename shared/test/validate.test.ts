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
