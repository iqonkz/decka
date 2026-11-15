import { describe, expect, test } from "vitest";
import { cellFromPoint, DRAG_THRESHOLD, dragReducer, IDLE, type DragEvent, type DragPayload, type DragState } from "../src/tabs/dragMachine";

const item: DragPayload = { type: "item", id: "a" };
const run = (events: DragEvent[], from: DragState = IDLE) => {
  let state = from;
  const results = [];
  for (const e of events) {
    const out = dragReducer(state, e);
    state = out.state;
    if (out.result) results.push(out.result);
  }
  return { state, results };
};

describe("dragReducer", () => {
  test("a press that does not move is a click", () => {
    const { state, results } = run([{ type: "down", payload: item, x: 10, y: 10 }, { type: "move", x: 12, y: 11 }, { type: "up", x: 12, y: 11 }]);
    expect(results).toEqual([{ kind: "click", payload: item }]);
    expect(state).toEqual(IDLE);
  });

  test("the threshold is 4 px: 3 px stays a press, 4 px starts the drag", () => {
    expect(DRAG_THRESHOLD).toBe(4);
    expect(run([{ type: "down", payload: item, x: 0, y: 0 }, { type: "move", x: 3, y: 0 }]).state.phase).toBe("pressed");
    expect(run([{ type: "down", payload: item, x: 0, y: 0 }, { type: "move", x: 4, y: 0 }]).state.phase).toBe("dragging");
    expect(run([{ type: "down", payload: item, x: 0, y: 0 }, { type: "move", x: 3, y: 3 }]).state.phase).toBe("dragging"); // 4.24 px
  });

  test("a drag ends in a drop at the last position", () => {
    const { state, results } = run([
      { type: "down", payload: item, x: 0, y: 0 },
      { type: "move", x: 20, y: 5 },
      { type: "move", x: 40, y: 8 },
      { type: "up", x: 41, y: 9 },
    ]);
    expect(results).toEqual([{ kind: "drop", payload: item, x: 41, y: 9 }]);
    expect(state).toEqual(IDLE);
  });

  test("Esc (cancel) abandons a pressed or a dragging gesture; nothing is dropped", () => {
    for (const moved of [false, true]) {
      const events: DragEvent[] = [{ type: "down", payload: item, x: 0, y: 0 }];
      if (moved) events.push({ type: "move", x: 50, y: 50 });
      events.push({ type: "cancel" });
      const { state, results } = run(events);
      expect(results).toEqual([{ kind: "cancel", payload: item }]);
      expect(state).toEqual(IDLE);
    }
  });

  test("while dragging, the position follows the pointer; a second press is ignored", () => {
    const { state } = run([
      { type: "down", payload: item, x: 0, y: 0 },
      { type: "move", x: 10, y: 0 },
      { type: "down", payload: { type: "widget", kind: "clock" }, x: 99, y: 99 },
      { type: "move", x: 30, y: 7 },
    ]);
    expect(state).toMatchObject({ phase: "dragging", payload: item, x: 30, y: 7 });
  });

  test("idle ignores move, up and cancel", () => {
    expect(run([{ type: "move", x: 1, y: 1 }, { type: "up", x: 1, y: 1 }, { type: "cancel" }])).toEqual({ state: IDLE, results: [] });
  });
});

describe("cellFromPoint", () => {
  const m = { offsetX: 10, offsetY: 20, side: 100, gap: 10 };
  const grid = { cols: 4, rows: 2 };
  test("finds the cell at scale 1 and at a smaller scale", () => {
    expect(cellFromPoint(15, 25, m, 1, grid)).toEqual({ x: 0, y: 0 });
    expect(cellFromPoint(10 + 110 * 2 + 5, 20 + 110 + 5, m, 1, grid)).toEqual({ x: 2, y: 1 });
    expect(cellFromPoint((10 + 110 * 3 + 50) * 0.5, (20 + 50) * 0.5, m, 0.5, grid)).toEqual({ x: 3, y: 0 });
  });
  test("outside the grid is null", () => {
    expect(cellFromPoint(2, 25, m, 1, grid)).toBeNull();
    expect(cellFromPoint(15, 5, m, 1, grid)).toBeNull();
    expect(cellFromPoint(10 + 110 * 4 + 5, 25, m, 1, grid)).toBeNull();
    expect(cellFromPoint(15, 20 + 110 * 2 + 5, m, 1, grid)).toBeNull();
  });
});