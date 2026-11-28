/**
 * The voice API of the phone (M21, TASKS T21.5b/T21.5): what the button, the bubble and the confirmation sheet use.
 * The sound goes to the PC as `voice_*` messages (session.ts); the state comes back in the `voice` section.
 */
import { useSyncExternalStore } from "react";
import { onPcLeave, onVoiceError, sendVoice } from "../app/controller";
import { getState, useAppState } from "../store/store";
import { startAudioCapture } from "./capture";
import { newVoiceId, VoiceSession, type VoiceMode } from "./session";

export interface VoiceInfo {
  state: "idle" | "listening" | "loading" | "recognizing" | "thinking" | "confirm" | "done" | "error";
  id?: string;
  source?: "pc" | "phone";
  heard?: string;
  reply?: string;
  confirm?: { text: string; key?: string; args?: Record<string, string> };
  replyKey?: string;
  replyArgs?: Record<string, string>;
}

/** Level of the microphone (0..1) and whether this phone records now: small stores for the button. */
let level = 0;
let recording = false;
const listeners = new Set<() => void>();
function subscribe(fn: () => void): () => void {
  listeners.add(fn);
  return () => listeners.delete(fn);
}
function setLocal(l: number, r: boolean): void {
  if (l === level && r === recording) return;
  level = l;
  recording = r;
  listeners.forEach((fn) => fn());
}

export const VOICE_NO_SPEECH_EVENT = "decka:voice:nospeech";
export const VOICE_MIC_DENIED_EVENT = "decka:voice:micdenied";
/** T32.H.3: `detail` is the key of the string that says why nothing will come of the request. */
export const VOICE_FAILED_EVENT = "decka:voice:failed";

const session = new VoiceSession({
  send: sendVoice,
  startCapture: (onChunk) => startAudioCapture({ onChunk }),
  newId: () => newVoiceId(),
  setTimeout: (fn, ms) => window.setTimeout(fn, ms),
  clearTimeout: (h) => window.clearTimeout(h as number),
  onLevel: (l) => setLocal(l, recording),
  onActive: (a) => setLocal(a ? level : 0, a),
  onNoSpeech: () => window.dispatchEvent(new Event(VOICE_NO_SPEECH_EVENT)),
  onMicDenied: () => window.dispatchEvent(new Event(VOICE_MIC_DENIED_EVENT)),
  onFailed: (reason) => window.dispatchEvent(new CustomEvent(VOICE_FAILED_EVENT, { detail: reason })),
});
onVoiceError((code, message) => session.refused(code, message));
// A recording belongs to the PC it was started on: it ends before the phone leaves that PC.
onPcLeave(() => session.cancel());

/** The level of the microphone for the ring of the button (0 when not recording). */
export function useVoiceLevel(): number {
  return useSyncExternalStore(subscribe, () => level);
}

/** Whether this phone is recording now (the button shows `stop`). */
export function useVoiceRecording(): boolean {
  return useSyncExternalStore(subscribe, () => recording);
}

/** Starts recording: `tap` ends by silence or the next tap, `hold` (walkie-talkie) only by `stopVoice`. Rejects when the PC is
 * not reachable or the microphone is refused (the UI shows a hint). */
export function startVoice(mode: VoiceMode = "tap"): Promise<void> {
  return session.start(mode);
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
