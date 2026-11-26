// The M29 settings are kept by the PC: what the window saves is what a new window reads, and a refusal is not swallowed.
import { describe, expect, it } from "vitest";
import type { IslandSettings } from "@decka/shared";
import { DEFAULT_EXTRAS, loadVoiceExtras, saveDuckMusic, saveIsland, type Call } from "../src/tabs/voiceExtras";

/** The agent behind the IPC: it outlives any window. */
function agent(refuse = false) {
  let duck = true;
  let island: IslandSettings = { ...DEFAULT_EXTRAS.island };
  const call = ((cmd: string, args?: Record<string, unknown>) => {
    if (refuse && cmd.endsWith("_set")) return Promise.reject("disk is full");
    if (cmd === "voice_duck_get") return Promise.resolve(duck);
    if (cmd === "voice_duck_set") return Promise.resolve((duck = args!.enabled as boolean));
    if (cmd === "island_config_get") return Promise.resolve(island);
    if (cmd === "island_config_set") return Promise.resolve((island = { ...island, ...(args as Partial<IslandSettings>) }));
    return Promise.reject(`unknown command ${cmd}`);
  }) as Call;
  return call;
}

describe("the voice and island settings", () => {
  it("are saved by the PC and read again by a new window", async () => {
    const pc = agent();
    expect(await loadVoiceExtras(pc)).toEqual(DEFAULT_EXTRAS);
    await saveDuckMusic(pc, false);
    await saveIsland(pc, { restingMode: "lyrics" });
    await saveIsland(pc, { barHeight: 14 });
    await saveIsland(pc, { barPosition: "left" });
    // The window is closed and opened again: nothing is remembered in the window itself.
    expect(await loadVoiceExtras(pc)).toEqual({ duckMusic: false, island: { restingMode: "lyrics", barHeight: 14, barPosition: "left" } });
  });

  it("report a refusal to the page", async () => {
    const pc = agent(true);
    await expect(saveDuckMusic(pc, false)).rejects.toBe("disk is full");
    await expect(saveIsland(pc, { barHeight: 8 })).rejects.toBe("disk is full");
  });
});
