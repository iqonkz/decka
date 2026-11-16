import { describe, expect, test } from "vitest";
import { BACK_LIMIT, popBack, pushBack } from "../src/deck/backStack";

const always = () => true;

describe("backStack (DATA_MODEL.md 3.8)", () => {
  test("push and pop are last in, first out", () => {
    const stack = pushBack(pushBack([], "main"), "dev");
    expect(stack).toEqual(["main", "dev"]);
    const first = popBack(stack, always);
    expect(first).toEqual({ pageId: "dev", stack: ["main"] });
    expect(popBack(first.stack, always)).toEqual({ pageId: "main", stack: [] });
  });

  test("an empty stack gives no page: the caller opens the first one", () => {
    expect(popBack([], always)).toEqual({ pageId: null, stack: [] });
  });

  test("the stack keeps the last 10; the oldest falls off", () => {
    let stack: string[] = [];
    for (let i = 1; i <= 12; i++) stack = pushBack(stack, `p${i}`);
    expect(stack).toHaveLength(BACK_LIMIT);
    expect(stack[0]).toBe("p3");
    expect(stack[9]).toBe("p12");
  });

  test("pages that no longer exist are skipped", () => {
    const gone = new Set(["b", "c"]);
    expect(popBack(["a", "b", "c"], (id) => !gone.has(id))).toEqual({ pageId: "a", stack: [] });
    expect(popBack(["b", "c"], (id) => !gone.has(id))).toEqual({ pageId: null, stack: [] });
  });

  test("the input is not changed", () => {
    const stack = ["a", "b"];
    pushBack(stack, "c");
    popBack(stack, always);
    expect(stack).toEqual(["a", "b"]);
  });
});
