import { describe, expect, test } from "vitest";
import actionsJson from "../fixtures/actions.default.json";
import layoutJson from "../fixtures/layout.default.json";
import { insertItem, isReservedVar, sizeAllowed, validateActions, validateLayout, type ActionsFile, type LayoutFile } from "../src";

const layout = layoutJson as unknown as LayoutFile;
const actions = actionsJson as unknown as ActionsFile;
const only = (action: Record<string, unknown>): ActionsFile => ({ version: 1, actions: { y: { name: "Y", ...action } as never } });

describe("the YouTube pieces", () => {
  test("the widget takes 2x1..4x2", () => {
    for (const [w, h, ok] of [[2, 1, true], [3, 1, true], [4, 2, true], [1, 1, false], [5, 1, false], [2, 3, false]] as const) {
      expect(sizeAllowed("youtube", w, h, { cols: 7, rows: 4 }), `${w}x${h}`).toBe(ok);
    }
  });

  test("a chat message has a text of 1..200 characters", () => {
    expect(validateActions(only({ type: "youtube", op: "chat", message: "Hi" }))).toEqual([]);
    for (const message of [undefined, "", "x".repeat(201)]) {
      expect(validateActions(only({ type: "youtube", op: "chat", ...(message === undefined ? {} : { message }) }))[0]?.path, `${message}`).toBe("/actions/y/message");
    }
  });

  test("youtube. variables belong to Decka", () => {
    expect(isReservedVar("youtube.viewers")).toBe(true);
    expect(isReservedVar("youtubers")).toBe(false);
  });

  test("the chat key asks for its text and keeps it in the action", () => {
    const ctx = { language: "en" as const, icons: {} };
    const page = layout.pages[0]!.id;
    const asked = insertItem(layout, actions, page, "youtube.chat", {}, ctx);
    expect(asked.ok ? Object.values(asked.actions.actions).some((a) => a.type === "youtube") : false).toBe(false);
    const done = insertItem(layout, actions, page, "youtube.chat", { text: "Thanks for watching" }, ctx);
    if (!done.ok) throw new Error(done.error);
    expect(Object.values(done.actions.actions).find((a) => a.type === "youtube")).toMatchObject({ type: "youtube", op: "chat", message: "Thanks for watching" });
    expect(validateActions(done.actions)).toEqual([]);
    expect(validateLayout(done.layout, done.actions)).toEqual([]);
  });
});
