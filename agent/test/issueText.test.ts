import { describe, expect, test } from "vitest";
import type { LayoutFile } from "@decka/shared";
import layoutDefault from "@decka/shared/fixtures/layout.default.json";
import { describeIssue } from "../src/tabs/issueText";

const layout = structuredClone(layoutDefault) as unknown as LayoutFile;
layout.pages[0]!.items.push({ id: "b-new", kind: "button", x: 0, y: 3, w: 1, h: 1, label: "New" });
const index = layout.pages[0]!.items.length - 1;

describe("describeIssue", () => {
  test("an item: page name, kind, 1-based column and row", () => {
    const text = describeIssue({ path: `/pages/0/items/${index}`, message: "choose an action or a page for this button" }, layout);
    expect(text).toBe('Page "Main", button at column 1, row 4: choose an action or a page for this button');
  });

  test("a page's apps", () => {
    expect(describeIssue({ path: "/pages/1/apps/0", message: "code.exe is already used" }, layout)).toBe(`Page "${layout.pages[1]!.name}": code.exe is already used`);
  });

  test("the grid", () => {
    expect(describeIssue({ path: "/grid", message: "too small" }, layout)).toBe("Grid: too small");
  });

  test("an unknown path stays as it is", () => {
    expect(describeIssue({ path: "/display/dimLevel", message: "bad" }, layout)).toBe("/display/dimLevel: bad");
    expect(describeIssue({ path: "", message: "bad" }, layout)).toBe("/: bad");
  });

  test("an item that no longer exists falls back to the path", () => {
    expect(describeIssue({ path: "/pages/9/items/9", message: "bad" }, layout)).toBe("/pages/9/items/9: bad");
  });
});
