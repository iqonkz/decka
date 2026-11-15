import { describe, expect, test } from "vitest";
import { validateActions, validateLayout } from "@decka/shared";
import type { ActionsFile, InstalledApp, LayoutFile } from "@decka/shared";
import { DEFAULT_PRESETS, planWizard } from "../src/editor/wizardPlan";

const layout = (cols = 4, rows = 2): LayoutFile => ({
  version: 1,
  grid: { cols, rows },
  display: { dimAfterSec: 60, dimLevel: 0.3, sleepAfterSec: 600, pixelShift: true, night: { enabled: false, from: "23:00", to: "07:00" } },
  pages: [{ id: "main", name: "Main", items: [{ id: "w-clock", kind: "clock", x: 0, y: 0, w: 2, h: 1 }] }],
});
const none: ActionsFile = { version: 1, actions: {} };
const app = (name: string): InstalledApp => ({ name, program: `C:\\Apps\\${name}.exe`, args: [] });

describe("planWizard", () => {
  test("programs and ready-made actions go into the free cells in reading order; the result validates", () => {
    const r = planWizard(layout(), none, [{ app: app("Spotify") }, { app: app("Chrome"), icon: "img:0123456789abcdef" }], ["play-pause", "mute"]);
    expect(r.placed).toBe(4);
    expect(r.skipped).toBe(0);
    const buttons = r.layout.pages[0]!.items.filter((i) => i.kind === "button");
    expect(buttons.map((b) => [b.x, b.y, (b as { label?: string }).label])).toEqual([
      [2, 0, "Spotify"],
      [3, 0, "Chrome"],
      [0, 1, "Play / Pause"],
      [1, 1, "Mute"],
    ]);
    expect(validateActions(r.actions)).toEqual([]);
    expect(validateLayout(r.layout, r.actions)).toEqual([]);
  });

  test("existing buttons stay where they are; what does not fit is counted", () => {
    const r = planWizard(layout(3, 1), none, [{ app: app("A") }, { app: app("B") }], DEFAULT_PRESETS);
    expect(r.layout.pages[0]!.items.filter((i) => i.kind === "button")).toHaveLength(1); // 3 columns, the clock takes 2
    expect(r.layout.pages[0]!.items.find((i) => i.id === "w-clock")).toMatchObject({ x: 0, y: 0, w: 2, h: 1 });
    expect(r.placed).toBe(1);
    expect(r.skipped).toBe(2 + DEFAULT_PRESETS.length - 1 - 0);
  });

  test("running it twice adds nothing new for actions that already exist (same content reused)", () => {
    const first = planWizard(layout(6, 3), none, [], ["play-pause"]);
    const second = planWizard(first.layout, first.actions, [], ["play-pause"]);
    expect(Object.keys(second.actions.actions)).toEqual(Object.keys(first.actions.actions));
    expect(second.placed).toBe(1);
  });

  test("unknown ready-made ids are ignored; the defaults are the five of DESIGN.md 4.7", () => {
    expect(planWizard(layout(), none, [], ["nope"])).toMatchObject({ placed: 0, skipped: 0 });
    expect(DEFAULT_PRESETS).toEqual(["play-pause", "next-track", "volume-up", "volume-down", "mute"]);
  });
});