import { describe, expect, test } from "vitest";
import {
  AudioChunker,
  CHUNK_SAMPLES,
  downsampleTo16k,
  floatTo16BitPCM,
} from "../src/voice/capture";

describe("downsampleTo16k", () => {
  test("downsamples 48 kHz to 16 kHz with exact 1/3 length", () => {
    // 4800 samples at 48 kHz = 100 ms
    const input48k = new Float32Array(4800);
    for (let i = 0; i < input48k.length; i++) {
      input48k[i] = Math.sin((2 * Math.PI * 440 * i) / 48000);
    }

    const out16k = downsampleTo16k(input48k, 48000);
    expect(out16k.length).toBe(1600);
    expect(out16k.length).toBe(CHUNK_SAMPLES);
  });

  test("preserves zeros when downsampling 48 kHz silence", () => {
    const silence = new Float32Array(4800);
    const out = downsampleTo16k(silence, 48000);
    expect(out.length).toBe(1600);
    for (let i = 0; i < out.length; i++) {
      expect(out[i]).toBe(0);
    }
  });

  test("downsamples 44.1 kHz to 16 kHz", () => {
    // 4410 samples at 44.1 kHz = 100 ms
    const input44k = new Float32Array(4410);
    const out16k = downsampleTo16k(input44k, 44100);
    expect(out16k.length).toBe(1600);
  });

  test("returns copy if already 16 kHz", () => {
    const input16k = new Float32Array([0.1, 0.2, 0.3]);
    const out = downsampleTo16k(input16k, 16000);
    expect(out.length).toBe(3);
    expect(out[0]).toBeCloseTo(0.1);
  });

  test("handles empty input", () => {
    const empty = new Float32Array(0);
    const out = downsampleTo16k(empty, 48000);
    expect(out.length).toBe(0);
  });
});

describe("floatTo16BitPCM", () => {
  test("converts extreme float values to 16-bit signed PCM", () => {
    const input = new Float32Array([0.0, 1.0, -1.0, 0.5, -0.5]);
    const pcm = floatTo16BitPCM(input);
    expect(pcm[0]).toBe(0);
    expect(pcm[1]).toBe(32767);
    expect(pcm[2]).toBe(-32768);
    expect(pcm[3]).toBe(Math.floor(0.5 * 32767));
    expect(pcm[4]).toBe(-16384);
  });

  test("clamps values outside [-1.0, 1.0]", () => {
    const input = new Float32Array([2.5, -3.0]);
    const pcm = floatTo16BitPCM(input);
    expect(pcm[0]).toBe(32767);
    expect(pcm[1]).toBe(-32768);
  });
});

describe("AudioChunker", () => {
  test("chunks stream into exact 100 ms (1600 samples) buffers", () => {
    const emittedChunks: Int16Array[] = [];
    const chunker = new AudioChunker((chunk) => {
      emittedChunks.push(chunk);
    }, 1600);

    // Push 3 batches of 1000 samples (total 3000 samples)
    const batch = new Int16Array(1000).fill(42);
    chunker.push(batch); // 1000 in buffer -> 0 chunks
    expect(emittedChunks.length).toBe(0);
    expect(chunker.pending).toBe(1000);

    chunker.push(batch); // 2000 in buffer -> 1 chunk (1600), 400 remaining
    expect(emittedChunks.length).toBe(1);
    expect(emittedChunks[0]?.length).toBe(1600);
    expect(chunker.pending).toBe(400);

    chunker.push(batch); // 1400 in buffer -> still 1 chunk
    expect(emittedChunks.length).toBe(1);
    expect(chunker.pending).toBe(1400);

    // Push 300 more -> total 1700 -> emits 2nd chunk, 100 pending
    chunker.push(new Int16Array(300).fill(42));
    expect(emittedChunks.length).toBe(2);
    expect(emittedChunks[1]?.length).toBe(1600);
    expect(chunker.pending).toBe(100);

    // Flush remaining 100
    chunker.flush();
    expect(emittedChunks.length).toBe(3);
    expect(emittedChunks[2]?.length).toBe(100);
    expect(chunker.pending).toBe(0);
  });
});
