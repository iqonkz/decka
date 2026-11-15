import { describe, expect, test } from "vitest";
import { validateActions, validateLayout } from "../src/validate";
import type { ActionsFile } from "../src/actions";
import type { LayoutFile } from "../src/layout";
import layoutDefault from "../fixtures/layout.default.json";
import layoutContext from "../fixtures/layout.context.json";
import layoutPopup from "../fixtures/layout.popup.json";
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

describe("validateLayout: popup groups (rule 12)", () => {
  const popupLayout = layoutPopup as unknown as LayoutFile;
  const clone = (): LayoutFile => structuredClone(popupLayout);
  /** The first problem a mutation of the popup fixture causes, as [path, message]. */
  const problems = (mutate: (l: LayoutFile) => void): [string, string][] => {
    const l = clone();
    mutate(l);
    return validateLayout(l, actions).map((i) => [i.path, i.message]);
  };
  const popupPage = (l: LayoutFile) => l.pages.find((p) => p.popup === true)!;
  const button = (l: LayoutFile, id: string) => l.pages.flatMap((p) => p.items).find((i) => i.id === id) as unknown as Record<string, unknown>;

  test("the fixture is valid", () => {
    expect(validateLayout(popupLayout, actions)).toEqual([]);
  });

  test("a key may run an action, open a page, open a group, or run an action and then open a group", () => {
    expect(problems((l) => Object.assign(button(l, "b-more"), { actionId: "media-play" }))).toEqual([]);
  });

  test("a button with nothing to do is an error", () => {
    expect(problems((l) => delete button(l, "b-more").popup)).toEqual([["/pages/0/items/14", "choose an action or a page for this button"]]);
  });

  test("a page and a group together are refused", () => {
    expect(problems((l) => Object.assign(button(l, "b-more"), { navigate: "dev" }))).toEqual([["/pages/0/items/14", "a button can open a page or a popup group, not both"]]);
  });

  test("an action and a page together are still refused", () => {
    expect(problems((l) => Object.assign(button(l, "b-to-dev"), { actionId: "media-play" }))).toEqual([["/pages/0/items/15", "a button can either run an action or open a page, not both"]]);
  });

  test("popup must name an existing popup page", () => {
    expect(problems((l) => Object.assign(button(l, "b-more"), { popup: "nowhere" }))).toEqual([["/pages/0/items/14", 'popup group "nowhere" no longer exists — choose another']]);
    expect(problems((l) => Object.assign(button(l, "b-more"), { popup: "dev" }))).toEqual([["/pages/0/items/14", 'page "dev" is not a popup group']]);
  });

  test("navigate cannot point at a popup page", () => {
    expect(problems((l) => Object.assign(button(l, "b-to-dev"), { navigate: "media-more" }))).toEqual([["/pages/0/items/15", 'page "media-more" is a popup group — choose a normal page']]);
  });

  test("the first page cannot be a popup group", () => {
    expect(problems((l) => (l.pages[0]!.popup = true)).map(([p]) => p)).toContain("/pages/0/popup");
  });

  test("a popup group cannot have apps", () => {
    expect(problems((l) => (popupPage(l).apps = ["code.exe"]))).toEqual([["/pages/2/apps", "a popup group cannot show itself for apps: remove them"]]);
  });

  test("a key inside a popup group cannot open a page or another group", () => {
    expect(
      problems((l) => {
        const b = button(l, "b-pop-up");
        delete b.actionId;
        b.navigate = "main";
      }),
    ).toEqual([["/pages/2/items/2", "a key inside a popup group cannot open a page or another group"]]);
    expect(problems((l) => Object.assign(button(l, "b-pop-up"), { popup: "media-more", actionId: "vol-up" }))).toEqual([["/pages/2/items/2", "a key inside a popup group cannot open a page or another group"]]);
  });

  test("stayOpen belongs to popup pages only", () => {
    expect(problems((l) => (popupPage(l).stayOpen = true))).toEqual([]);
    expect(problems((l) => (l.pages[1]!.stayOpen = true))).toEqual([["/pages/1/stayOpen", "stayOpen only makes sense for a popup group"]]);
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

describe("validateLayout: context pages and media widget (M11)", () => {
  const base = layoutDefault as LayoutFile;
  const page = (id: string, apps?: string[], items: unknown[] = []) => ({ id, name: id, items, ...(apps ? { apps } : {}) });
  const layoutOf = (pages: unknown[]): LayoutFile => ({ ...base, pages: pages as LayoutFile["pages"] });
  const media = (over: Record<string, unknown> = {}) => ({ id: "m", kind: "media", x: 0, y: 0, w: 3, h: 1, ...over });

  test("the context fixture is valid with the default actions", () => {
    expect(validateLayout(layoutContext as LayoutFile, actions)).toEqual([]);
  });

  test("one app on two pages: error on the second page", () => {
    const issues = validateLayout(layoutOf([page("a"), page("b", ["code.exe"]), page("c", ["code.exe"])]), actions);
    expect(issues.map((i) => i.path)).toEqual(["/pages/2/apps/0"]);
  });

  test("apps on the first page are an error", () => {
    expect(validateLayout(layoutOf([page("a", ["code.exe"]), page("b")]), actions).map((i) => i.path)).toEqual(["/pages/0/apps"]);
    expect(validateLayout(layoutOf([page("a", []), page("b")]), actions)).toEqual([]);
  });

  test("media: 1x1 and 3x3 are errors, 2x1 and 3x2 are fine", () => {
    expect(validateLayout(layoutOf([page("a", undefined, [media({ w: 1 })])]), actions)).toHaveLength(1);
    expect(validateLayout(layoutOf([page("a", undefined, [media({ h: 3 })])]), actions)).toHaveLength(1);
    expect(validateLayout(layoutOf([page("a", undefined, [media({ w: 2 })])]), actions)).toEqual([]);
    expect(validateLayout(layoutOf([page("a", undefined, [media({ h: 2 })])]), actions)).toEqual([]);
  });

  test("media with an unknown playAction is an error on the item", () => {
    const issues = validateLayout(layoutOf([page("a", undefined, [media({ playAction: "nope" })])]), actions);
    expect(issues.map((i) => i.path)).toEqual(["/pages/0/items/0"]);
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

describe("key face (M14)", () => {
  const clone = () => structuredClone(layoutDefault) as unknown as LayoutFile;
  test("known faces pass, an unknown face of a button or of the display is an error", () => {
    const ok = clone();
    ok.display.keyFace = "full";
    const first = ok.pages[0]!.items.find((i) => i.kind === "button")!;
    (first as { face?: string }).face = "full";
    expect(validateLayout(ok, actionsDefault as unknown as ActionsFile)).toEqual([]);

    const bad = clone();
    (bad.pages[0]!.items.find((i) => i.kind === "button") as { face?: string }).face = "huge";
    expect(validateLayout(bad, actionsDefault as unknown as ActionsFile).map((i) => i.message)).toContain("face must be standard or full");

    const badDisplay = clone();
    (badDisplay.display as { keyFace?: string }).keyFace = "huge";
    expect(validateLayout(badDisplay, actionsDefault as unknown as ActionsFile).map((i) => i.path)).toContain("/display/keyFace");
  });
});
