import { describe, expect, test } from "vitest";
import type { StatusItem, StatusState } from "@decka/shared";
import { sortStatuses } from "../src/deck/widgets/statusOrder";

const s = (id: string, label: string, state: StatusState, group?: string): StatusItem => {
  const item: StatusItem = { id, label, state, source: "push", updatedAt: 0 };
  if (group !== undefined) item.group = group;
  return item;
};

describe("sortStatuses", () => {
  const items = [
    s("a", "Alpha", "ok", "svc"),
    s("b", "Beta", "error", "svc"),
    s("c", "Gamma", "unknown", "ai"),
    s("d", "Delta", "running", "ai"),
    s("e", "Echo", "warn"),
    s("f", "Foxtrot", "error", "svc"),
  ];

  test("error, warn, running, unknown, ok; by label inside a state", () => {
    expect(sortStatuses(items).map((x) => x.id)).toEqual(["b", "f", "e", "d", "c", "a"]);
  });

  test("a group filter keeps only that group", () => {
    expect(sortStatuses(items, "ai").map((x) => x.id)).toEqual(["d", "c"]);
    expect(sortStatuses(items, "nope")).toEqual([]);
  });

  test("does not mutate its input", () => {
    const copy = [...items];
    sortStatuses(items);
    expect(items).toEqual(copy);
  });
});
