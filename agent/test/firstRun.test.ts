import { describe, expect, test } from "vitest";
import type { ActionsFile, InstalledApp, LayoutFile } from "@decka/shared";
import layoutDefault from "@decka/shared/fixtures/layout.default.json";
import { launchActionOf, newAppActionId, planAppButtons } from "../src/tabs/layoutOps";

const layout = layoutDefault as unknown as LayoutFile;
const actions: ActionsFile = { version: 1, actions: { "app-notepad": { name: "Old", type: "system", op: "lock" } } };
const app = (name: string, over: Partial<InstalledApp> = {}): InstalledApp => ({ name, program: `C:\\Apps\\${name}.exe`, args: [], ...over });

describe("newAppActionId", () => {
  test("a readable id from the name, numbered when taken", () => {
    expect(newAppActionId("Visual Studio Code", {})).toBe("app-visual-studio-code");
    expect(newAppActionId("Notepad", actions.actions)).toBe("app-notepad-2");
    expect(newAppActionId("Блокнот", {})).toBe("app-program");
    expect(newAppActionId("  --  ", {})).toBe("app-program");
  });
});

describe("launchActionOf", () => {
  test("carries the program, arguments and working folder of the shortcut", () => {
    expect(launchActionOf(app("Chrome", { args: ["--new-window"], cwd: "C:\\Apps" }))).toEqual({ name: "Chrome", type: "launch", program: "C:\\Apps\\Chrome.exe", args: ["--new-window"], cwd: "C:\\Apps" });
    expect(launchActionOf(app("Plain"))).toEqual({ name: "Plain", type: "launch", program: "C:\\Apps\\Plain.exe" });
  });
});

describe("planAppButtons (first-run wizard, step 3)", () => {
  test("each program becomes a launch action and a button in a free cell of the first page", () => {
    const plan = planAppButtons([app("Notepad"), app("Steam")], layout, actions, { "c:\\apps\\steam.exe": "img:0123456789abcdef" });
    expect(plan.placed).toBe(2);
    expect(plan.skipped).toBe(0);
    expect(Object.keys(plan.actions.actions).sort()).toEqual(["app-notepad", "app-notepad-2", "app-steam"]);
    const added = plan.layout.pages[0]!.items.slice(layout.pages[0]!.items.length);
    expect(added.map((i) => [i.x, i.y])).toEqual([[3, 3], [4, 3]]); // the free cells of the default page, in reading order
    expect(added[1]).toMatchObject({ kind: "button", label: "Steam", actionId: "app-steam", icon: "img:0123456789abcdef" });
    expect(added[0]).not.toHaveProperty("icon");
  });

  test("programs that find no free cell are skipped, not lost silently", () => {
    const many = Array.from({ length: 8 }, (_, i) => app(`App${i}`));
    const plan = planAppButtons(many, layout, { version: 1, actions: {} });
    expect(plan.placed).toBe(3); // (3,3) (4,3) (5,3) are free on the default first page
    expect(plan.skipped).toBe(5);
  });

  test("the input is not modified", () => {
    const before = JSON.stringify(layout);
    planAppButtons([app("X")], layout, actions);
    expect(JSON.stringify(layout)).toBe(before);
    expect(Object.keys(actions.actions)).toEqual(["app-notepad"]);
  });
});
