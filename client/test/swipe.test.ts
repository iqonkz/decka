import { describe, expect, test } from "vitest";
import { dragOffset, swipeDecision, targetPage, velocity } from "../src/deck/swipe";

describe("swipeDecision (threshold 25% of the width or 0.5 px/ms)", () => {
  test("a long slow drag commits", () => {
    expect(swipeDecision(-250, 0.1, 1000)).toBe("next");
    expect(swipeDecision(250, -0.1, 1000)).toBe("prev"); // far enough: the distance decides, whatever the speed
  });

  test("a short slow drag stays", () => {
    expect(swipeDecision(-249, 0.2, 1000)).toBe("stay");
    expect(swipeDecision(100, 0.1, 1000)).toBe("stay");
  });

  test("a short fast flick commits in its direction", () => {
    expect(swipeDecision(-40, -0.6, 1000)).toBe("next");
    expect(swipeDecision(40, 0.6, 1000)).toBe("prev");
  });

  test("speed against the drag direction does not count", () => {
    expect(swipeDecision(-40, 0.9, 1000)).toBe("stay");
  });

  test("exactly 0.5 px/ms is not fast; nothing moved stays", () => {
    expect(swipeDecision(-40, -0.5, 1000)).toBe("stay");
    expect(swipeDecision(0, 5, 1000)).toBe("stay");
    expect(swipeDecision(-500, -5, 0)).toBe("stay");
  });
});

describe("targetPage", () => {
  test("moves by one and never wraps", () => {
    expect(targetPage(1, "next", 3)).toBe(2);
    expect(targetPage(2, "next", 3)).toBe(2);
    expect(targetPage(1, "prev", 3)).toBe(0);
    expect(targetPage(0, "prev", 3)).toBe(0);
    expect(targetPage(1, "stay", 3)).toBe(1);
  });
});

describe("dragOffset", () => {
  test("follows the finger inside the range, gives way beyond the ends", () => {
    expect(dragOffset(-90, 0, 3)).toBe(-90);
    expect(dragOffset(90, 1, 3)).toBe(90);
    expect(dragOffset(-90, 2, 3)).toBe(-30);
    expect(dragOffset(90, 0, 3)).toBe(30);
  });
});

describe("velocity", () => {
  test("px per ms, zero when no time passed", () => {
    expect(velocity(0, 0, -50, 100)).toBe(-0.5);
    expect(velocity(0, 10, 30, 10)).toBe(0);
  });
});
