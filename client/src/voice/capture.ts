/**
 * Audio capture and downsampling for client voice input (M21, DATA_MODEL.md §12.6, TASKS.md A4).
 *
 * Captures microphone audio via `getUserMedia` and `AudioContext`,
 * downsamples to 16 kHz mono PCM16, and emits 100 ms chunks (1600 samples) via callback.
 */

export const TARGET_SAMPLE_RATE = 16000;
export const CHUNK_MS = 100;
export const CHUNK_SAMPLES = (TARGET_SAMPLE_RATE * CHUNK_MS) / 1000; // 1600 samples per 100 ms

/**
 * Resamples a Float32Array from `fromRate` to 16 kHz mono.
 */
export function downsampleTo16k(input: Float32Array, fromRate: number): Float32Array {
  if (fromRate === TARGET_SAMPLE_RATE || input.length === 0) {
    return new Float32Array(input);
  }

  // Integer decimation fast path (e.g. 48000 -> 16000, ratio = 3)
  if (fromRate > TARGET_SAMPLE_RATE && fromRate % TARGET_SAMPLE_RATE === 0) {
    const ratio = Math.floor(fromRate / TARGET_SAMPLE_RATE);
    const outLength = Math.floor(input.length / ratio);
    const output = new Float32Array(outLength);
    for (let i = 0; i < outLength; i++) {
      let sum = 0;
      const start = i * ratio;
      for (let j = 0; j < ratio; j++) {
        sum += input[start + j] ?? 0;
      }
      output[i] = sum / ratio;
    }
    return output;
  }

  // Linear interpolation for general rates (e.g. 44100 -> 16000)
  const outLength = Math.floor((input.length * TARGET_SAMPLE_RATE) / fromRate);
  const output = new Float32Array(outLength);
  for (let i = 0; i < outLength; i++) {
    const srcPos = (i * fromRate) / TARGET_SAMPLE_RATE;
    const idx = Math.floor(srcPos);
    const frac = srcPos - idx;
    const s1 = input[idx] ?? 0;
    const s2 = input[idx + 1] ?? s1;
    output[i] = s1 + frac * (s2 - s1);
  }
  return output;
}

/**
 * Converts Float32 audio samples in [-1.0, 1.0] to 16-bit signed PCM (Int16Array).
 */
export function floatTo16BitPCM(input: Float32Array): Int16Array {
  const output = new Int16Array(input.length);
  for (let i = 0; i < input.length; i++) {
    const s = Math.max(-1, Math.min(1, input[i] ?? 0));
    output[i] = s < 0 ? s * 0x8000 : s * 0x7fff;
  }
  return output;
}

/**
 * Buffers incoming 16 kHz PCM16 samples and emits exact chunks of `chunkSamples` (default 1600 = 100 ms).
 */
export class AudioChunker {
  private buffer: number[] = [];
  readonly chunkSize: number;
  private readonly onChunk: (chunk: Int16Array) => void;

  constructor(onChunk: (chunk: Int16Array) => void, chunkSize = CHUNK_SAMPLES) {
    this.onChunk = onChunk;
    this.chunkSize = chunkSize;
  }

  /**
   * Pushes new samples into the buffer and emits full chunks.
   */
  push(samples: Int16Array): void {
    for (let i = 0; i < samples.length; i++) {
      this.buffer.push(samples[i] ?? 0);
      if (this.buffer.length >= this.chunkSize) {
        const chunk = new Int16Array(this.buffer.slice(0, this.chunkSize));
        this.buffer = this.buffer.slice(this.chunkSize);
        this.onChunk(chunk);
      }
    }
  }

  /**
   * Flushes any remaining partial chunk.
   */
  flush(): void {
    if (this.buffer.length > 0) {
      const chunk = new Int16Array(this.buffer);
      this.buffer = [];
      this.onChunk(chunk);
    }
  }

  /** Current pending samples in buffer. */
  get pending(): number {
    return this.buffer.length;
  }
}

export interface AudioCaptureSession {
  stop: () => void;
}

export interface CaptureOptions {
  onChunk: (chunk: Int16Array) => void;
  onError?: (err: unknown) => void;
}

/**
 * Starts capturing audio from the default microphone, downsampling to 16 kHz PCM16,
 * and delivering 100 ms chunks to `options.onChunk`.
 */
export async function startAudioCapture(options: CaptureOptions): Promise<AudioCaptureSession> {
  const stream = await navigator.mediaDevices.getUserMedia({
    audio: {
      channelCount: 1,
      echoCancellation: true,
      noiseSuppression: true,
    },
  });

  const AudioContextClass = window.AudioContext || (window as unknown as { webkitAudioContext: typeof AudioContext }).webkitAudioContext;
  const audioCtx = new AudioContextClass();
  // A context made after the wait for the permission may start asleep: then no sound would ever arrive.
  if (audioCtx.state === "suspended") void audioCtx.resume().catch(() => {});
  const source = audioCtx.createMediaStreamSource(stream);
  const chunker = new AudioChunker(options.onChunk, CHUNK_SAMPLES);

  // Buffer size 4096 gives ~85 ms chunks at 48 kHz
  const bufferSize = 4096;
  const processor = audioCtx.createScriptProcessor(bufferSize, 1, 1);

  processor.onaudioprocess = (e) => {
    try {
      const input = e.inputBuffer.getChannelData(0);
      const resampled = downsampleTo16k(input, audioCtx.sampleRate);
      const pcm16 = floatTo16BitPCM(resampled);
      chunker.push(pcm16);
    } catch (err) {
      options.onError?.(err);
    }
  };

  source.connect(processor);
  processor.connect(audioCtx.destination);

  let stopped = false;
  return {
    stop: () => {
      if (stopped) return;
      stopped = true;
      try {
        chunker.flush();
        processor.disconnect();
        source.disconnect();
        stream.getTracks().forEach((track) => track.stop());
        void audioCtx.close();
      } catch {
        // ignore cleanup errors
      }
    },
  };
}
