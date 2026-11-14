import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type { AgentStatus, IpcCommands } from "@decka/shared";

/** Typed `invoke`: the command names and shapes come from `IpcCommands` (PROTOCOL.md 9). */
export function call<K extends keyof IpcCommands>(cmd: K, args?: Parameters<IpcCommands[K]>[0]): Promise<ReturnType<IpcCommands[K]>> {
  return invoke<ReturnType<IpcCommands[K]>>(cmd, args as Record<string, unknown> | undefined);
}

/** Subscribes to `decka://status-changed`; returns the unsubscribe function. */
export async function onStatus(fn: (status: AgentStatus) => void): Promise<() => void> {
  return listen<AgentStatus>("decka://status-changed", (event) => fn(event.payload));
}

/** Subscribes to `decka://navigate` (the tray asks the window to open a tab). */
export async function onNavigate(fn: (tab: string) => void): Promise<() => void> {
  return listen<string>("decka://navigate", (event) => fn(event.payload));
}
