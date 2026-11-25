/**
 * One voice request of the phone (M21, PROTOCOL.md 3): `voice_start`, chunks of 100 ms, `voice_end` / `voice_cancel`.
 * The phone sends sound and answers only; what to do with them is decided on the PC (ARCHITECTURE.md S24).
 * Everything the session touches (the channel, the microphone, the clock, the ids) is injected, so it runs in tests.
 */
import type { VoiceCancelMsg, VoiceChunkMsg, VoiceConfirmMsg, VoiceEndMsg, VoiceStartMsg } from "@decka/shared";
import { CHUNK_MS, chunkRms, Endpointer, LevelSmoother } from "./endpoint";

const FULL_CHUNK_SAMPLES = (16_000 * CHUNK_MS) / 1000;

export type VoiceMsg = VoiceStartMsg | VoiceChunkMsg | VoiceEndMsg | VoiceCancelMsg | VoiceConfirmMsg;

/** The agent cuts a recording at 30 s; the phone ends it a little earlier by itself (a walkie-talkie hold is the only long one). */
export const MAX_RECORDING_MS = 29_500;

/** `tap`: the end comes by silence (DATA_MODEL.md 12.6a) or the next tap; `hold`: walkie-talkie, only the release ends it. */
export type VoiceMode = "tap" | "hold";

export interface VoiceDeps {
  /** `false` when there is no live channel. */
  send(msg: VoiceMsg): boolean;
  startCapture(onChunk: (chunk: Int16Array) => void): Promise<{ stop: () => void }>;
  newId(): string;
  setTimeout(fn: () => void, ms: number): unknown;
  clearTimeout(handle: unknown): void;
  /** The smoothed level 0..1 of every 100 ms (the ring of the button); 0 when the recording is over. */
  onLevel?(level: number): void;
  /** A recording of this phone started (`true`) or is over (`false`). */
  onActive?(active: boolean): void;
  /** Nothing was said for 5 s: the request was cancelled and no sound went to the PC for recognition. */
  onNoSpeech?(): void;
  /** The microphone was refused by Android or the WebView: the button says how to allow it. */
  onMicDenied?(): void;
}

/** The error of getUserMedia when the permission is missing or refused. */
export function isMicDenied(e: unknown): boolean {
  const name = (e as { name?: string } | null)?.name ?? "";
  return name === "NotAllowedError" || name === "SecurityError" || name === "PermissionDeniedError";
}

interface Current {
  id: string;
  seq: number;
  capture: { stop: () => void } | null;
  timer: unknown;
  mode: VoiceMode;
  endpointer: Endpointer;
  smoother: LevelSmoother;
  /** The silence rule fired (once). */
  decided: boolean;
  /** A release or a second tap came while the microphone was still opening: the recording ends as soon as it is open. */
  stopRequested: boolean;
}

/** Base64 of the little-endian bytes of the samples (PCM16). */
export function chunkToBase64(chunk: Int16Array): string {
  const bytes = new Uint8Array(chunk.buffer, chunk.byteOffset, chunk.byteLength);
  let s = "";
  for (let i = 0; i < bytes.length; i++) s += String.fromCharCode(bytes[i]!);
  return btoa(s);
}

/** 16 lower-case hex characters. */
export function newVoiceId(
  fill: (n: Uint8Array<ArrayBuffer>) => void = (n) => {
    crypto.getRandomValues(n);
  },
): string {
  const bytes = new Uint8Array(8);
  fill(bytes);
  return Array.from(bytes, (b) => b.toString(16).padStart(2, "0")).join("");
}

export class VoiceSession {
  private cur: Current | null = null;

  constructor(private readonly d: VoiceDeps) {}

  get active(): boolean {
    return this.cur !== null;
  }

  /** Starts a recording. Rejects when the channel is down or the microphone is refused (the PC is told to forget it). */
  async start(mode: VoiceMode = "tap"): Promise<void> {
    if (this.cur) return;
    const id = this.d.newId();
    const cur: Current = { id, seq: 0, capture: null, timer: null, mode, endpointer: new Endpointer(), smoother: new LevelSmoother(), decided: false, stopRequested: false };
    if (!this.safeSend({ t: "voice_start", id })) throw new Error("voice: offline");
    this.cur = cur;
    this.d.onActive?.(true);
    let capture: { stop: () => void };
    try {
      capture = await this.d.startCapture((chunk) => this.push(cur, chunk));
    } catch (e) {
      if (this.cur === cur) {
        this.cur = null;
        this.d.onActive?.(false);
        this.safeSend({ t: "voice_cancel", id });
      }
      if (isMicDenied(e)) this.d.onMicDenied?.();
      throw e;
    }
    if (this.cur !== cur) {
      // Stopped or cancelled while the microphone was being opened (the permission question can take long).
      capture.stop();
      return;
    }
    cur.capture = capture;
    cur.timer = this.d.setTimeout(() => this.stop(), MAX_RECORDING_MS);
    if (cur.stopRequested) this.stop();
  }

  /** A socket that closes under our hands (send throws) is the same as no channel: never an exception out of the audio callback. */
  private safeSend(msg: VoiceMsg): boolean {
    try {
      return this.d.send(msg);
    } catch {
      return false;
    }
  }

  /** The end of speech: the rest of the sound goes out, then `voice_end`. */
  stop(): void {
    const cur = this.cur;
    if (!cur) return;
    if (!cur.capture) {
      // The microphone is still opening (the permission question, a slow start): the end waits for it. A short tap never gets here;
      // only a release of the walkie-talkie or a second tap does, and neither one cancels the request (DATA_MODEL.md 12.6).
      cur.stopRequested = true;
      return;
    }
    this.d.clearTimeout(cur.timer);
    cur.capture.stop(); // flushes the last partial chunk through `push`
    this.cur = null;
    this.d.onActive?.(false);
    this.d.onLevel?.(0);
    this.safeSend({ t: "voice_end", id: cur.id });
  }

  cancel(): void {
    const cur = this.cur;
    if (!cur) return;
    this.cur = null;
    this.d.clearTimeout(cur.timer);
    cur.capture?.stop();
    this.d.onActive?.(false);
    this.d.onLevel?.(0);
    this.safeSend({ t: "voice_cancel", id: cur.id });
  }

  /** The answer to the question of the PC about a risky call. */
  confirm(id: string, yes: boolean): void {
    this.safeSend({ t: "voice_confirm", id, yes });
  }

  private push(cur: Current, chunk: Int16Array): void {
    // After `cancel` the late chunks of the closing microphone are dropped; the flush inside `stop` still sees `this.cur === cur`.
    if (this.cur !== cur || chunk.length === 0) return;
    if (!this.safeSend({ t: "voice_chunk", id: cur.id, seq: cur.seq, data: chunkToBase64(chunk) })) {
      this.cancel();
      return;
    }
    cur.seq += 1;
    // Level and the end by silence follow whole chunks of 100 ms only (the flushed rest is shorter).
    if (chunk.length < FULL_CHUNK_SAMPLES) return;
    const rms = chunkRms(chunk);
    this.d.onLevel?.(cur.smoother.feed(rms));
    if (cur.mode !== "tap" || cur.decided) return;
    const verdict = cur.endpointer.feed(rms);
    if (verdict === "end") {
      cur.decided = true;
      this.stop();
    } else if (verdict === "no_speech") {
      cur.decided = true;
      this.cancel();
      this.d.onNoSpeech?.();
    }
  }
}
