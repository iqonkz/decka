import { describe, expect, it } from "vitest";
import { chunkToBase64, MAX_RECORDING_MS, newVoiceId, VoiceSession, type VoiceDeps, type VoiceMsg } from "./session";
import { pageIndexOf, parseGoto } from "./gotoMath";

function rig(opts: { online?: boolean; micFails?: boolean; extra?: Partial<VoiceDeps> } = {}) {
  const sent: VoiceMsg[] = [];
  const timers: { fn: () => void; ms: number; live: boolean }[] = [];
  let feed: (c: Int16Array) => void = () => {};
  let openMic: (() => void) | null = null;
  let stopped = 0;
  let online = opts.online ?? true;
  const deps: VoiceDeps = {
    send: (m) => (online ? (sent.push(m), true) : false),
    startCapture: (onChunk) => {
      feed = onChunk;
      if (opts.micFails) return Promise.reject(new Error("denied"));
      return new Promise((resolve) => {
        openMic = () => resolve({ stop: () => { stopped++; feed(new Int16Array([7, 8])); } });
      });
    },
    newId: () => "0011223344556677",
    setTimeout: (fn, ms) => (timers.push({ fn, ms, live: true }), timers.length - 1),
    clearTimeout: (h) => { const t = timers[h as number]; if (t) t.live = false; },
    ...opts.extra,
  };
  return { s: new VoiceSession(deps), sent, timers, feed: (c: Int16Array) => feed(c), open: () => openMic?.(), stopped: () => stopped, offline: () => { online = false; } };
}

const tick = () => new Promise((r) => setTimeout(r, 0));

describe("voice session", () => {
  it("start, chunks with seq from 0, then end", async () => {
    const r = rig();
    const p = r.s.start();
    r.open();
    await p;
    r.feed(new Int16Array(1600));
    r.feed(new Int16Array(1600));
    r.s.stop();
    expect(r.sent.map((m) => m.t)).toEqual(["voice_start", "voice_chunk", "voice_chunk", "voice_chunk", "voice_end"]);
    const seqs = r.sent.filter((m) => m.t === "voice_chunk").map((m) => (m as { seq: number }).seq);
    expect(seqs).toEqual([0, 1, 2]); // the third is the flushed rest
    expect(r.s.active).toBe(false);
  });

  it("a second start while one runs does nothing", async () => {
    const r = rig();
    const p = r.s.start();
    r.open();
    await p;
    await r.s.start();
    expect(r.sent.filter((m) => m.t === "voice_start")).toHaveLength(1);
  });

  it("the recording ends by itself before the agent's 30 s limit", async () => {
    const r = rig();
    const p = r.s.start();
    r.open();
    await p;
    const t = r.timers.find((x) => x.live)!;
    expect(t.ms).toBe(MAX_RECORDING_MS);
    expect(MAX_RECORDING_MS).toBeLessThan(30_000);
    t.fn();
    expect(r.sent.at(-1)).toEqual({ t: "voice_end", id: "0011223344556677" });
  });

  it("a tap shorter than the microphone needs to open is cancelled, and a late microphone is closed", async () => {
    const r = rig();
    const p = r.s.start();
    r.s.stop();
    expect(r.sent.map((m) => m.t)).toEqual(["voice_start", "voice_cancel"]);
    r.open();
    await p;
    expect(r.stopped()).toBe(1);
    expect(r.sent).toHaveLength(2);
  });

  it("a refused microphone cancels the request on the PC and rejects", async () => {
    const r = rig({ micFails: true });
    await expect(r.s.start()).rejects.toThrow("denied");
    expect(r.sent.map((m) => m.t)).toEqual(["voice_start", "voice_cancel"]);
    expect(r.s.active).toBe(false);
  });

  it("offline: nothing starts", async () => {
    const r = rig({ online: false });
    await expect(r.s.start()).rejects.toThrow("offline");
    expect(r.s.active).toBe(false);
  });

  it("the channel dropping in the middle cancels locally and sends no more chunks", async () => {
    const r = rig();
    const p = r.s.start();
    r.open();
    await p;
    r.offline();
    r.feed(new Int16Array(1600));
    expect(r.s.active).toBe(false);
    expect(r.stopped()).toBe(1);
  });

  it("cancel drops the late chunks of the closing microphone", async () => {
    const r = rig();
    const p = r.s.start();
    r.open();
    await p;
    r.s.cancel();
    expect(r.sent.map((m) => m.t)).toEqual(["voice_start", "voice_cancel"]);
  });

  it("confirm sends only the id and the answer", async () => {
    const r = rig();
    r.s.confirm("aabbccddeeff0011", true);
    expect(r.sent).toEqual([{ t: "voice_confirm", id: "aabbccddeeff0011", yes: true }]);
    await tick();
  });

  it("chunks are base64 of little-endian PCM16 and ids are 16 hex characters", () => {
    expect(chunkToBase64(new Int16Array([1, -2]))).toBe(btoa(String.fromCharCode(1, 0, 0xfe, 0xff)));
    expect(newVoiceId((n) => void n.fill(0xab))).toBe("abababababababab");
    expect(newVoiceId()).toMatch(/^[0-9a-f]{16}$/);
  });
});

describe("voice.goto", () => {
  it("accepts only <pageId>#<counter>", () => {
    expect(parseGoto("media#3")).toBe("media");
    for (const bad of [null, 5, "media", "media#", "../x#1", "a b#1", "MEDIA#1", "x#1;y"]) expect(parseGoto(bad)).toBeNull();
  });

  it("finds a page by id and never a popup", () => {
    const pages = [{ id: "main" }, { id: "pop", popup: true }, { id: "media" }];
    expect(pageIndexOf(pages, "media")).toBe(2);
    expect(pageIndexOf(pages, "pop")).toBe(-1);
    expect(pageIndexOf(pages, "none")).toBe(-1);
  });
});

describe("voice session: end by silence (12.6a)", () => {
  const loud = () => new Int16Array(1600).fill(8000);
  const quiet = () => new Int16Array(1600);
  const open = async (r: ReturnType<typeof rig>, mode?: "tap" | "hold") => {
    const p = r.s.start(mode);
    r.open();
    await p;
  };

  it("a tap recording ends by itself 2 s after the speech and sends voice_end", async () => {
    const r = rig();
    await open(r, "tap");
    for (let i = 0; i < 3; i++) r.feed(quiet());
    for (let i = 0; i < 5; i++) r.feed(loud());
    for (let i = 0; i < 25; i++) r.feed(quiet());
    expect(r.sent.filter((m) => m.t === "voice_end")).toHaveLength(1);
    expect(r.sent.filter((m) => m.t === "voice_cancel")).toHaveLength(0);
    expect(r.s.active).toBe(false);
  });

  it("nothing said for 5 s cancels, sends no end and says so", async () => {
    let said = 0;
    const r = rig({ extra: { onNoSpeech: () => { said++; } } });
    await open(r, "tap");
    for (let i = 0; i < 55; i++) r.feed(quiet());
    expect(r.sent.at(-1)?.t).toBe("voice_cancel");
    expect(r.sent.some((m) => m.t === "voice_end")).toBe(false);
    expect(said).toBe(1);
  });

  it("the walkie-talkie (hold) is not ended by silence", async () => {
    const r = rig();
    await open(r, "hold");
    for (let i = 0; i < 3; i++) r.feed(quiet());
    for (let i = 0; i < 5; i++) r.feed(loud());
    for (let i = 0; i < 60; i++) r.feed(quiet());
    expect(r.s.active).toBe(true);
    r.s.stop();
    expect(r.sent.at(-1)?.t).toBe("voice_end");
  });

  it("reports the level of every chunk and 0 when over", async () => {
    const levels: number[] = [];
    const r = rig({ extra: { onLevel: (l) => levels.push(l) } });
    await open(r, "hold");
    r.feed(loud());
    r.s.stop();
    expect(levels[0]).toBeGreaterThan(0.5);
    expect(levels.at(-1)).toBe(0);
  });
});
