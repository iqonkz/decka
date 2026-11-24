import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, test } from "vitest";
import type { LayoutItem, ListsSection, RemindersSection } from "@decka/shared";
import { DeckEnvContext, EMPTY_DATA, type DeckData, type DeckEnv } from "../src/deckEnv";
import { DeckMetricsContext } from "../src/metricsContext";
import Tile from "../src/Tile";
import { activeGuesses, listRows, NO_GUESS, tapped } from "../src/widgets/listLogic";
import { formatReminderTime } from "../src/widgets/remindersTime";

const NOOP = () => {};
const render = (item: LayoutItem, data: Partial<DeckData> = {}) => {
  const env: DeckEnv = { ...EMPTY_DATA, online: true, ...data, onPress: () => Promise.resolve(), onPressError: NOOP, onNavigate: NOOP, onDismiss: NOOP };
  return renderToStaticMarkup(
    <DeckEnvContext.Provider value={env}>
      <DeckMetricsContext.Provider value={{ side: 100, gap: 4 }}>
        <Tile item={item} />
      </DeckMetricsContext.Provider>
    </DeckEnvContext.Provider>,
  );
};
const at = (kind: string, w: number, h: number, extra: object = {}) => ({ id: "w1", kind, x: 0, y: 0, w, h, ...extra }) as LayoutItem;

const A = "aaaaaaaaaaaaaaaa";
const B = "bbbbbbbbbbbbbbbb";
const reminders: RemindersSection = {
  items: [
    { id: "0000000000000001", text: "Call mom", at: "2026-11-25T18:30" },
    { id: "0000000000000002", text: "Pay rent", at: "2026-11-26T09:00" },
  ],
};
const lists: ListsSection = {
  lists: [
    { id: A, name: "Shopping", items: [{ id: "1000000000000001", text: "milk", done: true }, { id: "1000000000000002", text: "bread", done: false }, { id: "1000000000000003", text: "eggs", done: false }] },
    { id: B, name: "Empty one", items: [] },
  ],
};

describe("reminders widget", () => {
  test("shows time and text, today as HH:mm and another day with the weekday", () => {
    const html = render(at("reminders", 2, 2), { reminders });
    expect(html).toContain("Reminders");
    expect(html).toContain("Call mom");
    expect(html).toContain("Pay rent");
    const now = new Date(2026, 10, 25, 12, 0);
    expect(formatReminderTime("2026-11-25T18:30", now, "en")).toBe("18:30");
    expect(formatReminderTime("2026-11-26T09:00", now, "en")).toBe("Thu 09:00");
    expect(formatReminderTime("2026-11-26T09:00", now, "ru")).toMatch(/^чт,? 09:00$/);
    expect(formatReminderTime("nonsense", now, "en")).toBe("nonsense");
  });

  test("empty state", () => {
    expect(render(at("reminders", 2, 1), { reminders: { items: [] } })).toContain("No reminders");
    expect(render(at("reminders", 2, 1), {})).toContain("No reminders");
  });

  test("a 2x1 widget shows fewer rows than a 2x2 one, each text on one line (ellipsis in CSS)", () => {
    const many: RemindersSection = { items: Array.from({ length: 5 }, (_, i) => ({ id: `000000000000000${i}`, text: `r${i}`, at: "2026-11-25T18:30" })) };
    const rows = (html: string) => (html.match(/class="rem-row"/g) ?? []).length;
    expect(rows(render(at("reminders", 2, 1), { reminders: many }))).toBe(2);
    expect(rows(render(at("reminders", 2, 2), { reminders: many }))).toBe(5);
    expect(rows(render(at("reminders", 4, 4), { reminders: many }))).toBe(5);
  });
});

describe("list widget", () => {
  test("shows the list by id, open items first and done ones struck through at the end", () => {
    const html = render(at("list", 2, 2, { listId: A }), { lists });
    expect(html).toContain("Shopping");
    expect(html.indexOf("bread")).toBeLessThan(html.indexOf("milk"));
    expect(html.indexOf("eggs")).toBeLessThan(html.indexOf("milk"));
    expect(html).toContain("list-done");
  });

  test("messages: no such list, an empty list, no lists at all", () => {
    expect(render(at("list", 2, 2, { listId: "cccccccccccccccc" }), { lists })).toContain("List not found");
    expect(render(at("list", 2, 2, { listId: B }), { lists })).toContain("List is empty");
    expect(render(at("list", 2, 2), { lists: { lists: [] } })).toContain("List is empty");
    expect(render(at("list", 2, 2), { lists })).toContain("Shopping"); // no listId: the first list
  });

  test("a tap marks at once and the next patch replaces the guess (a refused toggle rolls back)", () => {
    const items = lists.lists[0]!.items;
    const g1 = tapped(NO_GUESS, lists, "1000000000000002", false);
    const shown = listRows(items, activeGuesses(g1, lists));
    expect(shown.map((r) => [r.text, r.done])).toEqual([["eggs", false], ["milk", true], ["bread", true]]);
    // The PC answered with a section where nothing changed: a new object, so the guess is dropped.
    const patched: ListsSection = JSON.parse(JSON.stringify(lists));
    expect(listRows(patched.lists[0]!.items, activeGuesses(g1, patched)).map((r) => [r.text, r.done])).toEqual([["bread", false], ["eggs", false], ["milk", true]]);
    // A second tap on the same item undoes the guess against the same section.
    const g2 = tapped(g1, lists, "1000000000000002", true);
    expect(listRows(items, activeGuesses(g2, lists)).map((r) => r.text)).toEqual(["bread", "eggs", "milk"]);
  });
});