import { describe, expect, test } from "vitest";
import type { LayoutFile, LayoutItem } from "@decka/shared";
import { addToFirstFree } from "../src/tabs/layoutOps";
import { highlightParts } from "../src/tabs/librarySearch";

const button = (id: string, x: number, y: number, w = 1, h = 1) => ({ id, kind: "button", x, y, w, h, label: id }) as unknown as LayoutItem;
const layout = (items: LayoutItem[], cols = 3, rows = 2): LayoutFile =>
  ({ version: 1, grid: { cols, rows }, pages: [{ id: "main", name: "Main", items }] }) as unknown as LayoutFile;

describe("addToFirstFree", () => {
  test("the first free cell, row by row", () => {
    const r = addToFirstFree(layout([button("a", 0, 0), button("b", 1, 0)]), "main", button("n", 9, 9));
    expect(r.ok).toBe(true);
    if (r.ok) expect(r.layout.pages[0]?.items.map((i) => [i.id, i.x, i.y])).toEqual([["a", 0, 0], ["b", 1, 0], ["n", 2, 0]]);
  });
  test("a wide widget needs a free run", () => {
    const r = addToFirstFree(layout([button("a", 1, 0), button("b", 1, 1)]), "main", button("w", 0, 0, 2, 1));
    expect(r.ok).toBe(false);
  });
  test("a full page gives the error", () => {
    const full = layout([0, 1, 2].flatMap((x) => [0, 1].map((y) => button(`k${x}${y}`, x, y))));
    expect(addToFirstFree(full, "main", button("n", 0, 0))).toEqual({ ok: false, error: "No free cell on this page" });
    expect(addToFirstFree(full, "nope", button("n", 0, 0)).ok).toBe(false);
  });
});

describe("highlightParts", () => {
  test("matches are bold, in any case", () => {
    expect(highlightParts("Volume Mute", "mut")).toEqual([{ text: "Volume ", bold: false }, { text: "Mut", bold: true }, { text: "e", bold: false }]);
    expect(highlightParts("aXa", "x")).toEqual([{ text: "a", bold: false }, { text: "X", bold: true }, { text: "a", bold: false }]);
  });
  test("no query, no match", () => {
    expect(highlightParts("Mute", "")).toEqual([{ text: "Mute", bold: false }]);
    expect(highlightParts("Mute", "zzz")).toEqual([{ text: "Mute", bold: false }]);
  });
});
