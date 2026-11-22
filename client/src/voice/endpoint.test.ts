import { describe, expect, test } from "vitest";
import { chunkRms, Endpointer, LevelSmoother, type Verdict } from "./endpoint";

const QUIET = 0.002;
const SPEECH = 0.1;

/** The 1-based index of the first verdict that is not `continue`. */
function run(chunks: number[]): [number, Verdict] | null {
  const e = new Endpointer();
  for (let i = 0; i < chunks.length; i++) {
    const v = e.feed(chunks[i]!);
    if (v !== "continue") return [i + 1, v];
  }
  return null;
}
const rep = (v: number, n: number) => Array<number>(n).fill(v);

describe("end of the phrase (same constants as the PC)", () => {
  test("silence after speech ends after two seconds", () => {
    expect(run([...rep(QUIET, 3), ...rep(SPEECH, 10), ...rep(QUIET, 30)])).toEqual([33, "end"]);
  });

  test("noise without speech is no_speech at five seconds", () => {
    expect(run(rep(0.02, 80))).toEqual([50, "no_speech"]);
    expect(run(rep(QUIET, 80))).toEqual([50, "no_speech"]);
  });

  test("a pause of one second does not cut the phrase", () => {
    const c = [...rep(QUIET, 3), ...rep(SPEECH, 5), ...rep(QUIET, 10), ...rep(SPEECH, 5), ...rep(QUIET, 19)];
    expect(run(c)).toBeNull();
    expect(run([...c, QUIET])).toEqual([c.length + 1, "end"]);
  });

  test("a click is not speech", () => {
    expect(run([...rep(QUIET, 3), ...rep(SPEECH, 2), ...rep(QUIET, 60)])).toEqual([50, "no_speech"]);
  });

  test("twenty seconds end the recording", () => {
    expect(run([...rep(QUIET, 3), ...rep(SPEECH, 300)])).toEqual([200, "end"]);
  });

  test("the wave rises at once and falls in about 150 ms", () => {
    const s = new LevelSmoother();
    expect(s.feed(0.1)).toBeCloseTo(0.6, 5);
    const after = s.feed(0);
    expect(after).toBeGreaterThan(0.25);
    expect(after).toBeLessThan(0.35);
    expect(s.feed(1)).toBe(1);
  });

  test("rms of PCM16", () => {
    expect(chunkRms(new Int16Array(0))).toBe(0);
    expect(chunkRms(new Int16Array(1600))).toBe(0);
    expect(chunkRms(new Int16Array(1600).fill(16384))).toBeCloseTo(0.5, 5);
  });

  test("speech from the first chunk still ends two seconds after it", () => {
    expect(run([...rep(SPEECH, 10), ...rep(QUIET, 30)])).toEqual([30, "end"]);
  });

  test("a microphone that raises its gain in the pause still ends the phrase", () => {
    expect(run([...rep(QUIET, 3), ...rep(SPEECH, 10), ...rep(0.02, 30)])?.[1]).toBe("end");
  });});
