// Every window of the agent that listens to events needs a capability: without one Tauri refuses `listen` and the window stays
// blank (the highlight of the screen help did, from M27 to 0.51.0).
import { describe, expect, it } from "vitest";
import settings from "../src-tauri/capabilities/default.json";
import overlays from "../src-tauri/capabilities/island.json";
import highlight from "../src-tauri/src/highlight.rs?raw";
import island from "../src-tauri/src/island.rs?raw";
import tray from "../src-tauri/src/tray.rs?raw";

const labels = (source: string) => [...source.matchAll(/const LABEL: &str = "([\w-]+)"/g)].map((m) => m[1]!);

describe("the capabilities", () => {
  it("name every window the agent opens", () => {
    const opened = ["settings", ...labels(highlight), ...labels(island), ...labels(tray)];
    expect(opened).toEqual(expect.arrayContaining(["settings", "highlight", "island"]));
    for (const label of opened) {
      const given = [settings, overlays].filter((c) => c.windows.includes(label));
      expect(given.length, label).toBeGreaterThan(0);
      expect(given.some((c) => c.permissions.includes("core:default")), label).toBe(true);
    }
  });

  it("give dialogs, updates and restarts to the settings window only", () => {
    expect(overlays.permissions).toEqual(["core:default"]);
    expect(settings.windows).toEqual(["settings"]);
  });
});
