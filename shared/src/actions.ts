import type { Id, VarName } from "./protocol";

export interface ActionsFile { version: 1; actions: Record<Id, Action>; }

interface ActionBase {
  name: string;          // 1..60 chars, shown in the settings window
  toggleVar?: VarName;   // after a SUCCESSFUL run the agent inverts this boolean variable (missing = true)
}
export type Action =
  | (ActionBase & { type: "hotkey"; keys: KeyName[] })                 // 1..5 keys
  | (ActionBase & { type: "media"; op: MediaOp })
  | (ActionBase & { type: "launch"; program: string; args?: string[]; cwd?: string; hidden?: boolean })
  | (ActionBase & { type: "open_url"; url: string })
  | (ActionBase & { type: "system"; op: "lock" | "sleep" });

export type MediaOp = "play_pause" | "next" | "prev" | "vol_up" | "vol_down" | "mute";
export type KeyName = string; // see DATA_MODEL.md 2.2
