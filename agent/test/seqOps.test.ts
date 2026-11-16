import { describe, expect, test } from "vitest";
import { dropIndex, moveStep } from "../src/editor/seqOps";

describe("moveStep", () => {
  test("moves an item down and up", () => {
    expect(moveStep(["a", "b", "c", "d"], 0, 2)).toEqual(["b", "c", "a", "d"]);
    expect(moveStep(["a", "b", "c", "d"], 3, 1)).toEqual(["a", "d", "b", "c"]);
  });
  test("clamps the target and ignores a bad source", () => {
    expect(moveStep(["a", "b", "c"], 0, 9)).toEqual(["b", "c", "a"]);
    expect(moveStep(["a", "b", "c"], 2, -4)).toEqual(["c", "a", "b"]);
    const same = ["a", "b"];
    expect(moveStep(same, 5, 0)).toBe(same);
    expect(moveStep(same, 1, 1)).toBe(same);
  });
});

describe("dropIndex", () => {
  test("is the first row whose middle is below the pointer, the last row past the end", () => {
    const mids = [10, 30, 50];
    expect(dropIndex(mids, 0)).toBe(0);
    expect(dropIndex(mids, 20)).toBe(1);
    expect(dropIndex(mids, 49)).toBe(2);
    expect(dropIndex(mids, 99)).toBe(2);
    expect(dropIndex([], 5)).toBe(0);
  });
});
