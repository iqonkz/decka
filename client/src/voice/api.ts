/**
 * The voice API of the phone (M21, TASKS T21.5b/T21.5): what the button, the bubble and the confirmation sheet use.
 * The sound goes to the PC as `voice_*` messages (session.ts); the state comes back in the `voice` section.
 */
import { sendVoice } from "../app/controller";
import { getState, useAppState } from "../store/store";
import { startAudioCapture } from "./capture";
import { newVoiceId, VoiceSession } from "./session";

export interface VoiceInfo {
  state: "idle" | "listening" | "loading" | "thinking" | "confirm" | "done" | "error";
  id?: string;
  source?: "pc" | "phone";
  heard?: string;
  reply?: string;
  confirm?: { text: string; key?: string; args?: Record<string, string> };
  replyKey?: string;
  replyArgs?: Record<string, string>;
}

const session = new VoiceSession({
  send: sendVoice,
  startCapture: (onChunk) => startAudioCapture({ onChunk }),
  newId: () => newVoiceId(),
  setTimeout: (fn, ms) => window.setTimeout(fn, ms),
  clearTimeout: (h) => window.clearTimeout(h as number),
});

/** Starts recording. Rejects when the PC is not reachable or the microphone is refused (the UI shows a hint). */
export function startVoice(): Promise<void> {
  return session.start();
}

/** The end of speech (release, second tap). */
export function stopVoice(): void {
  session.stop();
}

export function cancelVoice(): void {
  session.cancel();
}

/** Yes or No to the question of the PC; only the request of this phone can be answered. */
export function confirmVoice(yes: boolean): void {
  const v = getState().state?.voice;
  if (v?.state === "confirm" && v.id) session.confirm(v.id, yes);
}

export function useVoiceState(): VoiceInfo | null {
  return useAppState((s) => ((s.state as any)?.voice as VoiceInfo) ?? null);
}

export function voiceReady(): boolean {
  return getState().state?.vars?.["voice.ready"] === true;
}
