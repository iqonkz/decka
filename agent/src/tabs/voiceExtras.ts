// M29: the settings of the voice and the island that live in `config.json` (T29.3, T29.5). They are always read from the PC
// when the page opens and written through IPC; the PC's answer is what the page shows, and a failure is reported.
import type { IpcCommands, IslandSettings } from "@decka/shared";

export type Call = <K extends keyof IpcCommands>(cmd: K, args?: Parameters<IpcCommands[K]>[0]) => Promise<ReturnType<IpcCommands[K]>>;

export interface VoiceExtras {
  duckMusic: boolean;
  island: IslandSettings;
}

export const DEFAULT_EXTRAS: VoiceExtras = { duckMusic: true, island: { restingMode: "bar", barHeight: 10, barPosition: "center" } };

export async function loadVoiceExtras(call: Call): Promise<VoiceExtras> {
  const [duckMusic, island] = await Promise.all([call("voice_duck_get"), call("island_config_get")]);
  return { duckMusic, island };
}

export const saveDuckMusic = (call: Call, enabled: boolean): Promise<boolean> => call("voice_duck_set", { enabled });

export const saveIsland = (call: Call, change: Parameters<IpcCommands["island_config_set"]>[0]): Promise<IslandSettings> => call("island_config_set", change);
