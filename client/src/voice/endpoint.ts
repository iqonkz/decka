/**
 * End of the phrase by silence (DATA_MODEL.md §12.6a). A pure function over the RMS of 100 ms chunks (PCM normalised to 1.0):
 * `continue | end | no_speech`. The PC has the same logic with the same constants in `agent/core/src/voice/endpoint.rs`.
 */

export const CHUNK_MS = 100;
/** The first 300 ms measure the noise floor. */
export const CALIBRATION_MS = 300;
/** Speech is a chunk louder than `max(floor * NOISE_FACTOR, MIN_THRESHOLD)`. */
export const NOISE_FACTOR = 3;
export const MIN_THRESHOLD = 0.015;
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
  private floorSum = 0;
  private floorN = 0;
  private threshold = 0;
  private speechMs = 0;
  private silenceMs = 0;

  /** The RMS of the next chunk of 100 ms. */
  feed(rms: number): Verdict {
    this.elapsedMs += CHUNK_MS;
    const calibrationChunks = CALIBRATION_MS / CHUNK_MS;
    if (this.floorN < calibrationChunks) {
      this.floorSum += rms;
      this.floorN += 1;
      if (this.floorN === calibrationChunks) {
        this.threshold = Math.max((this.floorSum / this.floorN) * NOISE_FACTOR, MIN_THRESHOLD);
      }
    } else if (rms > this.threshold) {
      this.speechMs += CHUNK_MS;
      this.silenceMs = 0;
    } else if (this.speechMs >= MIN_SPEECH_MS) {
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
