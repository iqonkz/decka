/**
 * One voice request of the phone (M21, PROTOCOL.md 3): `voice_start`, chunks of 100 ms, `voice_end` / `voice_cancel`.
 * The phone sends sound and answers only; what to do with them is decided on the PC (ARCHITECTURE.md S24).
 * Everything the session touches (the channel, the microphone, the clock, the ids) is injected, so it runs in tests.
 */
import type { VoiceCancelMsg, VoiceChunkMsg, VoiceConfirmMsg, VoiceEndMsg, VoiceStartMsg } from "@decka/shared";

export type VoiceMsg = VoiceStartMsg | VoiceChunkMsg | VoiceEndMsg | VoiceCancelMsg | VoiceConfirmMsg;

/** The agent cuts a recording at 30 s; the phone ends it a little earlier by itself. */
export const MAX_RECORDING_MS = 29_500;

export interface VoiceDeps {
  /** `false` when there is no live channel. */
  send(msg: VoiceMsg): boolean;
  startCapture(onChunk: (chunk: Int16Array) => void): Promise<{ stop: () => void }>;
  newId(): string;
  setTimeout(fn: () => void, ms: number): unknown;
  clearTimeout(handle: unknown): void;
}

interface Current {
  id: string;
  seq: number;
  capture: { stop: () => void } | null;
  timer: unknown;
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
  async start(): Promise<void> {
    if (this.cur) return;
    const id = this.d.newId();
    const cur: Current = { id, seq: 0, capture: null, timer: null };
    if (!this.d.send({ t: "voice_start", id })) throw new Error("voice: offline");
    this.cur = cur;
    let capture: { stop: () => void };
    try {
      capture = await this.d.startCapture((chunk) => this.push(cur, chunk));
    } catch (e) {
      if (this.cur === cur) {
        this.cur = null;
        this.d.send({ t: "voice_cancel", id });
      }
      throw e;
    }
    if (this.cur !== cur) {
      // Stopped or cancelled while the microphone was being opened (the permission question can take long).
      capture.stop();
      return;
    }
    cur.capture = capture;
    cur.timer = this.d.setTimeout(() => this.stop(), MAX_RECORDING_MS);
  }

  /** The end of speech: the rest of the sound goes out, then `voice_end`. */
  stop(): void {
    const cur = this.cur;
    if (!cur) return;
    if (!cur.capture) {
      // Nothing was recorded yet: a tap shorter than the microphone needs to open.
      this.cancel();
      return;
    }
    this.d.clearTimeout(cur.timer);
    cur.capture.stop(); // flushes the last partial chunk through `push`
    this.cur = null;
    this.d.send({ t: "voice_end", id: cur.id });
  }

  cancel(): void {
    const cur = this.cur;
    if (!cur) return;
    this.cur = null;
    this.d.clearTimeout(cur.timer);
    cur.capture?.stop();
    this.d.send({ t: "voice_cancel", id: cur.id });
  }

  /** The answer to the question of the PC about a risky call. */
  confirm(id: string, yes: boolean): void {
    this.d.send({ t: "voice_confirm", id, yes });
  }

  private push(cur: Current, chunk: Int16Array): void {
    // After `cancel` the late chunks of the closing microphone are dropped; the flush inside `stop` still sees `this.cur === cur`.
    if (this.cur !== cur || chunk.length === 0) return;
    if (!this.d.send({ t: "voice_chunk", id: cur.id, seq: cur.seq, data: chunkToBase64(chunk) })) {
      this.cancel();
      return;
    }
    cur.seq += 1;
  }
}
