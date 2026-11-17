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
  | (ActionBase & { type: "system"; op: "lock" | "sleep" })
  | (ActionBase & { type: "audio"; op: AudioOp })                                               // M15
  | (ActionBase & { type: "obs"; op: ObsOp; scene?: string; input?: string })                   // M15
  | (ActionBase & { type: "twitch"; op: TwitchOp; message?: string })                           // `message`: the text of an announcement, only there
  | (ActionBase & { type: "youtube"; op: YoutubeOp; message: string })                          // a message in the chat of the live stream, 1..200
  | (ActionBase & { type: "sequence"; steps: SequenceStep[] });                                 // M15

export type MediaOp = "play_pause" | "next" | "prev" | "vol_up" | "vol_down" | "mute" | "shuffle" | "repeat"; // shuffle, repeat: M15
export type AudioOp = "mic_toggle" | "out_mute_toggle";
export type ObsOp = "record_toggle" | "stream_toggle" | "scene" | "input_mute_toggle" | "save_replay";
export type TwitchOp = "clip" | "announce";
export type YoutubeOp = "chat";
export type SequenceStep = { actionId: Id } | { delayMs: number };
export type KeyName = string; // see DATA_MODEL.md 2.2

/** The limits of a `sequence` (DATA_MODEL.md 2.3). */
export const SEQUENCE_LIMITS = { steps: 10, delayMin: 50, delayMax: 10000, delaySum: 15000 } as const;

/** Variables set by the collectors; a user cannot write them (PROTOCOL.md 4, HTTP rules) or toggle them. */
export const RESERVED_VAR_PREFIXES = ["audio.", "mic.", "media.", "obs.", "ai.", "twitch.", "youtube."] as const;
export function isReservedVar(name: string): boolean {
  return RESERVED_VAR_PREFIXES.some((p) => name.startsWith(p));
}
