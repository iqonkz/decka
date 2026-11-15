import { describe, expect, test } from "vitest";
import { placeMenu } from "../src/editor/placeMenu";

const view = { w: 1000, h: 800 };
const size = { w: 220, h: 300 };

describe("placeMenu", () => {
  test("opens at the click when it fits", () => {
    expect(placeMenu({ x: 100, y: 120 }, size, view)).toEqual({ left: 100, top: 120 });
  });
  test("moves inside the window at the right and bottom edges", () => {
    expect(placeMenu({ x: 950, y: 100 }, size, view)).toEqual({ left: 776, top: 100 });
    expect(placeMenu({ x: 100, y: 780 }, size, view)).toEqual({ left: 100, top: 496 });
    expect(placeMenu({ x: 999, y: 799 }, size, view)).toEqual({ left: 776, top: 496 });
  });
  test("never goes past the top-left margin, even for a menu bigger than the window", () => {
    expect(placeMenu({ x: -10, y: -10 }, size, view)).toEqual({ left: 4, top: 4 });
    expect(placeMenu({ x: 50, y: 50 }, { w: 2000, h: 2000 }, view)).toEqual({ left: 4, top: 4 });
  });
});