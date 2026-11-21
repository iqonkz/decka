import { getState, useAppState } from "../store/store";

export interface VoiceInfo {
  state: "idle" | "listening" | "loading" | "thinking" | "confirm" | "done" | "error";
  id?: string;
  source?: "pc" | "phone";
  heard?: string;
  reply?: string;
  confirm?: { text: string };
}

export async function startVoice(): Promise<void> {
  throw new Error("voice: not implemented");
}

export function stopVoice(): void {
  throw new Error("voice: not implemented");
}

export function cancelVoice(): void {
  throw new Error("voice: not implemented");
}

export function confirmVoice(_yes: boolean): void {
  throw new Error("voice: not implemented");
}

export function useVoiceState(): VoiceInfo | null {
  return useAppState((s) => ((s.state as any)?.voice as VoiceInfo) ?? null);
}

export function voiceReady(): boolean {
  return getState().state?.vars?.["voice.ready"] === true;
}
