/**
 * End of the phrase by silence (DATA_MODEL.md §12.6a). A pure function over the RMS of 100 ms chunks (PCM normalised to 1.0):
 * `continue | end | no_speech`. The PC has the same logic with the same constants in `agent/core/src/voice/endpoint.rs`.
 */

export const CHUNK_MS = 100;
/** The noise floor is a low percentile of the last `FLOOR_WINDOW` chunks, so speech from the first second cannot raise it. */
export const FLOOR_WINDOW = 30;
export const FLOOR_PERCENTILE = 0.2;
/** Speech is a chunk louder than `clamp(floor * NOISE_FACTOR, MIN_THRESHOLD, MAX_THRESHOLD)`. */
export const NOISE_FACTOR = 3;
export const MIN_THRESHOLD = 0.015;
export const MAX_THRESHOLD = 0.03;
/** After speech, a chunk is silence below this share of the mean speech level. */
export const END_RATIO = 0.25;
/** Speech counts after this much of it. */
export const MIN_SPEECH_MS = 300;
/** This much silence in a row after speech ends the phrase. */
export const END_SILENCE_MS = 2000;
/** No speech by then: nothing was said. */
export const NO_SPEECH_MS = 5000;
/** A recording is never longer. */
export const MAX_MS = 20_000;

export type Verdict = "continue" | "end" | "no_speech";

export class Endpointer {
  private elapsedMs = 0;
  private window: number[] = [];
  private speechMs = 0;
  private silenceMs = 0;
  private speechSum = 0;
  private speechN = 0;

  private floor(): number {
    const v = [...this.window].sort((a, b) => a - b);
    return v[Math.floor(Math.max(0, v.length - 1) * FLOOR_PERCENTILE)] ?? 0;
  }

  /** The RMS of the next chunk of 100 ms. */
  feed(rms: number): Verdict {
    this.elapsedMs += CHUNK_MS;
    this.window.push(rms);
    if (this.window.length > FLOOR_WINDOW) this.window.shift();
    const start = Math.min(Math.max(this.floor() * NOISE_FACTOR, MIN_THRESHOLD), MAX_THRESHOLD);
    const inSpeech = this.speechMs >= MIN_SPEECH_MS;
    const threshold = inSpeech ? Math.max(start, (END_RATIO * this.speechSum) / Math.max(1, this.speechN)) : start;
    if (rms > threshold) {
      this.speechMs += CHUNK_MS;
      this.silenceMs = 0;
      this.speechSum += rms;
      this.speechN += 1;
    } else if (inSpeech) {
      this.silenceMs += CHUNK_MS;
    }
    if (this.elapsedMs >= MAX_MS || this.silenceMs >= END_SILENCE_MS) return "end";
    if (this.speechMs < MIN_SPEECH_MS && this.elapsedMs >= NO_SPEECH_MS) return "no_speech";
    return "continue";
  }
}
/** The wave of the button ring: rises at once, falls with a time constant of 150 ms. */
export class LevelSmoother {
  /** RMS of voice at a phone microphone is 0.03..0.2; the gain brings that to most of the range. */
  static readonly GAIN = 6;
  private level = 0;

  feed(rms: number): number {
    const raw = Math.max(0, Math.min(1, rms * LevelSmoother.GAIN));
    this.level = Math.max(raw, this.level * Math.exp(-CHUNK_MS / 150));
    return this.level;
  }
}

/** RMS of PCM16 samples, normalised to 0..1. */
export function chunkRms(chunk: Int16Array): number {
  if (chunk.length === 0) return 0;
  let sum = 0;
  for (let i = 0; i < chunk.length; i++) {
    const v = chunk[i]! / 32768;
    sum += v * v;
  }
  return Math.sqrt(sum / chunk.length);
}
