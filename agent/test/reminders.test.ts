import { describe, expect, test } from "vitest";
import type { ListRow, ReminderRow } from "@decka/shared";
import { editItems, isLocalTime, nextHour, visibleReminders } from "../src/tabs/remindersModel";
import { fromLines, toLines } from "../src/tabs/DictationBlock";

const list: ListRow = { id: "aaaaaaaaaaaaaaaa", name: "Shopping", items: [{ id: "1", text: "milk", done: false }, { id: "2", text: "bread", done: true }] };

describe("Reminders window model (DATA_MODEL.md 13.7)", () => {
  test("finished reminders are hidden until Show done, soonest first", () => {
    const rows: ReminderRow[] = [
      { id: "a", text: "late", at: "2026-11-26T09:00", done: false },
      { id: "b", text: "old", at: "2026-11-24T09:00", done: true },
      { id: "c", text: "soon", at: "2026-11-25T09:00", done: false },
    ];
    expect(visibleReminders(rows, false).map((r) => r.text)).toEqual(["soon", "late"]);
    expect(visibleReminders(rows, true).map((r) => r.text)).toEqual(["old", "soon", "late"]);
  });

  test("a list is sent whole: edited, added and removed items keep the ids of the others", () => {
    expect(editItems(list, { id: "1", done: true })).toEqual([{ id: "1", text: "milk", done: true }, { id: "2", text: "bread", done: true }]);
    expect(editItems(list, { id: "2", text: "rye bread" })).toEqual([{ id: "1", text: "milk", done: false }, { id: "2", text: "rye bread", done: true }]);
    expect(editItems(list, { id: "1", text: null })).toEqual([{ id: "2", text: "bread", done: true }]);
    expect(editItems(list, { text: "eggs" })).toEqual([{ id: "1", text: "milk", done: false }, { id: "2", text: "bread", done: true }, { text: "eggs", done: false }]);
    expect(editItems(list, { text: "" })).toHaveLength(2);
  });

  test("the time of a new reminder is a local time the PC reads", () => {
    expect(isLocalTime("2026-11-25T18:30")).toBe(true);
    expect(isLocalTime("2026-11-25 18:30")).toBe(false);
    expect(isLocalTime("")).toBe(false);
    // An hour ahead, rounded up to :00 or :30.
    expect(nextHour(new Date(2026, 10, 25, 14, 20))).toBe("2026-11-25T15:30");
    expect(nextHour(new Date(2026, 10, 25, 14, 0))).toBe("2026-11-25T15:00");
    expect(nextHour(new Date(2026, 10, 25, 14, 40))).toBe("2026-11-25T16:00");
    expect(nextHour(new Date(2026, 11, 31, 23, 40))).toBe("2027-01-01T01:00");
  });

  test("the dictionary is one phrase per line", () => {
    expect(fromLines("  Kazakhstan \r\n\r\nDecka\n  \nOpenAI  ")).toEqual(["Kazakhstan", "Decka", "OpenAI"]);
    expect(toLines(["a", "b"])).toBe("a\nb");
    expect(fromLines(toLines(["Алматы", "Decka"]))).toEqual(["Алматы", "Decka"]);
  });
});