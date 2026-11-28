/**
 * One voice request of the phone (M21, PROTOCOL.md 3): `voice_start`, chunks of 100 ms, `voice_end` / `voice_cancel`.
 * The phone sends sound and answers only; what to do with them is decided on the PC (ARCHITECTURE.md S24).
 * Everything the session touches (the channel, the microphone, the clock, the ids) is injected, so it runs in tests.
 */
import type { VoiceCancelMsg, VoiceChunkMsg, VoiceConfirmMsg, VoiceEndMsg, VoiceStartMsg } from "@decka/shared";
import { CHUNK_MS, chunkRms, Endpointer, LevelSmoother } from "./endpoint";

const FULL_CHUNK_SAMPLES = (16_000 * CHUNK_MS) / 1000;

export type VoiceMsg = VoiceStartMsg | VoiceChunkMsg | VoiceEndMsg | VoiceCancelMsg | VoiceConfirmMsg;

/** Samples of one `voice_chunk`: 2880 bytes. With its base64 the message stays under the 4096 bytes the PC takes in one
 * frame (PROTOCOL.md 1; a full 100 ms of 3200 bytes does not fit), and 11 of them a second stay under its 15. */
export const PACKET_SAMPLES = 1440;

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
  /** T32.H.3: nothing will come of the request; `reason` is the key of the string that says why. */
  onFailed?(reason: VoiceFailure): void;
}

export type VoiceFailure = "voice.reply.off_on_pc" | "voice.reply.busy" | "voice.reply.pc_refused" | "voice.reply.offline_pc" | "voice.reply.mic_unavailable";

/** The error of getUserMedia when the permission is missing or refused. */
export function isMicDenied(e: unknown): boolean {
  const name = (e as { name?: string } | null)?.name ?? "";
  return name === "NotAllowedError" || name === "SecurityError" || name === "PermissionDeniedError";
}

interface Current {
  id: string;
  seq: number;
  /** Sound that does not fill a message yet. */
  held: Int16Array;
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
  /** The id of the latest request, and whether a refusal of the PC was already told about it. */
  private last: string | null = null;
  private told = false;

  constructor(private readonly d: VoiceDeps) {}

  get active(): boolean {
    return this.cur !== null;
  }

  /** Starts a recording. Rejects when the channel is down or the microphone is refused (the PC is told to forget it). */
  async start(mode: VoiceMode = "tap"): Promise<void> {
    if (this.cur) return;
    const id = this.d.newId();
    const cur: Current = { id, seq: 0, held: new Int16Array(0), capture: null, timer: null, mode, endpointer: new Endpointer(), smoother: new LevelSmoother(), decided: false, stopRequested: false };
    if (!this.safeSend({ t: "voice_start", id })) {
      this.d.onFailed?.("voice.reply.offline_pc");
      throw new Error("voice: offline");
    }
    this.cur = cur;
    this.last = id;
    this.told = false;
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
      else this.d.onFailed?.("voice.reply.mic_unavailable");
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
    if (this.cur !== cur || (cur.held.length > 0 && !this.packet(cur, cur.held))) return;
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

  /** An error of the PC about a `voice_*` message (PROTOCOL.md 3). The first one of a request says why; the recording
   * stops, as nothing of it is taken any more. */
  refused(code: string, message: string): void {
    if (this.last === null || this.told) return;
    this.told = true;
    this.cancel();
    this.d.onFailed?.(code === "NOT_FOUND" ? "voice.reply.off_on_pc" : message.startsWith("another voice request") ? "voice.reply.busy" : "voice.reply.pc_refused");
  }

  /** The answer to the question of the PC about a risky call. */
  confirm(id: string, yes: boolean): void {
    this.safeSend({ t: "voice_confirm", id, yes });
  }

  /** One message of sound; a channel that is gone ends the request. */
  private packet(cur: Current, samples: Int16Array): boolean {
    if (!this.safeSend({ t: "voice_chunk", id: cur.id, seq: cur.seq, data: chunkToBase64(samples) })) {
      this.cancel();
      this.d.onFailed?.("voice.reply.offline_pc");
      return false;
    }
    cur.seq += 1;
    return true;
  }

  private push(cur: Current, chunk: Int16Array): void {
    // After `cancel` the late chunks of the closing microphone are dropped; the flush inside `stop` still sees `this.cur === cur`.
    if (this.cur !== cur || chunk.length === 0) return;
    const sound = new Int16Array(cur.held.length + chunk.length);
    sound.set(cur.held);
    sound.set(chunk, cur.held.length);
    let at = 0;
    for (; sound.length - at >= PACKET_SAMPLES; at += PACKET_SAMPLES) {
      if (!this.packet(cur, sound.subarray(at, at + PACKET_SAMPLES))) return;
    }
    cur.held = sound.slice(at);
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
