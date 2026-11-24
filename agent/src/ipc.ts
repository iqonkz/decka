import { invoke } from "@tauri-apps/api/core";
import { emit, listen } from "@tauri-apps/api/event";
import type { AgentStatus, IpcCommands, VoiceStatus } from "@decka/shared";

export { listen };

/** Typed `invoke`: the command names and shapes come from `IpcCommands` (PROTOCOL.md 9). */
export function call<K extends keyof IpcCommands>(cmd: K, args?: Parameters<IpcCommands[K]>[0]): Promise<ReturnType<IpcCommands[K]>> {
  return invoke<ReturnType<IpcCommands[K]>>(cmd, args as Record<string, unknown> | undefined);
}

/** Subscribes to `voice-status`; returns the unsubscribe function. */
export async function onVoiceStatus(fn: (status: VoiceStatus) => void): Promise<() => void> {
  return listen<VoiceStatus>("voice-status", (event) => fn(event.payload));
}

export interface VoicePillEvent {
  state: "idle" | "listening" | "loading" | "recognizing" | "thinking" | "confirm" | "done" | "error";
  id?: string;
  source?: "pc" | "phone";
  heard?: string;
  reply?: string;
  confirm?: { text: string; key?: string; args?: Record<string, string> };
  replyKey?: string;
  replyArgs?: Record<string, string>;
  /** `dictate`: the pill says `Dictating...` with a microphone; `screen`: `Looking at the screen` with an eye. */
  activity?: "dictate" | "screen";
  /** The chord shown by the idle state, e.g. `Ctrl+Alt+Space`. */
  hotkey?: string;
  level?: number;
}

/** Subscribes to `voice-pill`; returns the unsubscribe function. */
export async function onVoicePill(fn: (data: VoicePillEvent) => void): Promise<() => void> {
  return listen<VoicePillEvent>("voice-pill", (event) => fn(event.payload));
}

/** Subscribes to `decka://status-changed`; returns the unsubscribe function. */
export async function onStatus(fn: (status: AgentStatus) => void): Promise<() => void> {
  return listen<AgentStatus>("decka://status-changed", (event) => fn(event.payload));
}

/** Asks the window to open a tab (`"actions"`) or a tab with a hint (`"actions:new"`); the tray uses the same event. */
export function navigate(target: string): Promise<void> {
  return emit("decka://navigate", target);
}

/** Subscribes to `decka://navigate` (the tray asks the window to open a tab). */
export async function onNavigate(fn: (tab: string) => void): Promise<() => void> {
  return listen<string>("decka://navigate", (event) => fn(event.payload));
}
