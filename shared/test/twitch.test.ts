import { describe, expect, test } from "vitest";
import actionsJson from "../fixtures/actions.default.json";
import layoutJson from "../fixtures/layout.default.json";
import { insertItem, isReservedVar, sizeAllowed, validateActions, validateLayout, type ActionsFile, type LayoutFile } from "../src";

const layout = layoutJson as unknown as LayoutFile;
const actions = actionsJson as unknown as ActionsFile;
const only = (action: Record<string, unknown>): ActionsFile => ({ version: 1, actions: { t: { name: "T", ...action } as never } });

describe("the Twitch pieces", () => {
  test("the widget takes 2x1..4x2", () => {
    for (const [w, h, ok] of [[2, 1, true], [3, 1, true], [4, 2, true], [1, 1, false], [5, 1, false], [2, 3, false]] as const) {
      expect(sizeAllowed("twitch", w, h, { cols: 7, rows: 4 }), `${w}x${h}`).toBe(ok);
    }
  });

  test("the text goes with an announcement and with nothing else", () => {
    expect(validateActions(only({ type: "twitch", op: "clip" }))).toEqual([]);
    expect(validateActions(only({ type: "twitch", op: "announce", message: "Hi" }))).toEqual([]);
    expect(validateActions(only({ type: "twitch", op: "announce" }))[0]?.path).toBe("/actions/t/message");
    expect(validateActions(only({ type: "twitch", op: "clip", message: "x" }))[0]?.path).toBe("/actions/t/message");
    expect(validateActions(only({ type: "twitch", op: "announce", message: "x".repeat(501) }))).not.toEqual([]);
  });

  test("twitch. variables belong to Decka", () => {
    expect(isReservedVar("twitch.viewers")).toBe(true);
    expect(isReservedVar("twitchy")).toBe(false);
  });

  test("an announcement key asks for its text and keeps it in the action", () => {
    const ctx = { language: "en" as const, icons: {} };
    const page = layout.pages[0]!.id;
    // Without the text nothing is made (the window asks for it first).
    const asked = insertItem(layout, actions, page, "twitch.announce", {}, ctx);
    expect(asked.ok ? Object.values(asked.actions.actions).some((a) => a.type === "twitch") : false).toBe(false);
    const done = insertItem(layout, actions, page, "twitch.announce", { text: "Thanks for watching" }, ctx);
    if (!done.ok) throw new Error(done.error);
    const made = Object.values(done.actions.actions).find((a) => a.type === "twitch");
    expect(made).toMatchObject({ type: "twitch", op: "announce", message: "Thanks for watching" });
    expect(validateActions(done.actions)).toEqual([]);
    expect(validateLayout(done.layout, done.actions)).toEqual([]);
  });
});
