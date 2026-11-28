import { describe, expect, it } from "vitest";
import { chunkToBase64, isMicDenied, MAX_RECORDING_MS, newVoiceId, VoiceSession, type VoiceDeps, type VoiceMsg } from "./session";
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

const loud = () => new Int16Array(1600).fill(8000);
const quiet = () => new Int16Array(1600);
const open = async (r: ReturnType<typeof rig>, mode?: "tap" | "hold") => {
  const p = r.s.start(mode);
  r.open();
  await p;
};

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

  it("every message of sound fits one frame of 4096 bytes, nothing is lost and no more than 15 go in a second (T32.H.3)", async () => {
    const r = rig();
    await open(r, "hold");
    for (let i = 0; i < 20; i++) r.feed(Int16Array.from({ length: 1600 }, (_, n) => i * 100 + (n % 100)));
    r.s.stop();
    const chunks = r.sent.filter((m) => m.t === "voice_chunk") as { seq: number; data: string }[];
    expect(Math.max(...chunks.map((m) => JSON.stringify(m).length))).toBeLessThanOrEqual(4096);
    // 2 s of sound and the 2 samples the microphone gave up at its end.
    expect(chunks.reduce((sum, m) => sum + atob(m.data).length, 0)).toBe(2 * 32000 + 4);
    expect(chunks.map((m) => m.seq)).toEqual(chunks.map((_, i) => i));
    expect(chunks.length / 2).toBeLessThan(15);
    expect(r.sent.at(-1)?.t).toBe("voice_end");
  });

  it("a socket that throws on send (closing under the recording) cancels the recording, nothing escapes (T25.2)", async () => {
    let boom = false;
    const r = rig({ extra: { send: (m) => { if (boom) throw new Error("socket closing"); return (void m, true); } } });
    const p = r.s.start();
    r.open();
    await p;
    boom = true;
    expect(() => r.feed(new Int16Array(1600))).not.toThrow();
    expect(r.s.active).toBe(false);
    expect(() => r.s.stop()).not.toThrow();
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

  it("a tap (down and up within 100 ms) keeps recording: no voice_cancel, no voice_end", async () => {
    const r = rig();
    const p = r.s.start("tap"); // the button only starts on a tap; the release does not call stop()
    r.open();
    await p;
    r.feed(new Int16Array(1600));
    expect(r.s.active).toBe(true);
    expect(r.sent.map((m) => m.t)).toEqual(["voice_start", "voice_chunk"]);
  });

  it("a hold released before the microphone is open ends the request when it opens (voice_end, not voice_cancel)", async () => {
    const r = rig();
    const p = r.s.start("hold");
    r.s.stop();
    expect(r.sent.map((m) => m.t)).toEqual(["voice_start"]);
    r.open();
    await p;
    expect(r.sent.map((m) => m.t)).toEqual(["voice_start", "voice_chunk", "voice_end"]);
    expect(r.s.active).toBe(false);
  });

  it("a hold of 400 ms released with the microphone open sends voice_end", async () => {
    const r = rig();
    const p = r.s.start("hold");
    r.open();
    await p;
    r.feed(new Int16Array(1600));
    r.s.stop();
    expect(r.sent.at(-1)).toEqual({ t: "voice_end", id: "0011223344556677" });
    expect(r.sent.some((m) => m.t === "voice_cancel")).toBe(false);
  });

  it("a cancel while the microphone is opening still closes the late microphone", async () => {
    const r = rig();
    const p = r.s.start();
    r.s.cancel();
    r.open();
    await p;
    expect(r.stopped()).toBe(1);
    expect(r.sent.map((m) => m.t)).toEqual(["voice_start", "voice_cancel"]);
  });

  it("a refused microphone cancels the request on the PC and rejects", async () => {
    const r = rig({ micFails: true });
    await expect(r.s.start()).rejects.toThrow("denied");
    expect(r.sent.map((m) => m.t)).toEqual(["voice_start", "voice_cancel"]);
    expect(r.s.active).toBe(false);
  });

  it("a refused permission tells the button (and the PC is told to forget the request)", async () => {
    let denied = 0;
    const r = rig({ micFails: true, extra: { onMicDenied: () => denied++ } });
    await expect(r.s.start()).rejects.toThrow();
    expect(denied).toBe(0); // the rig rejects with a plain Error: not a permission problem
    expect(isMicDenied(Object.assign(new Error("x"), { name: "NotAllowedError" }))).toBe(true);
    expect(isMicDenied(Object.assign(new Error("x"), { name: "NotFoundError" }))).toBe(false);
    const sent: string[] = [];
    const s = new VoiceSession({
      send: (m) => (sent.push(m.t), true),
      startCapture: () => Promise.reject(Object.assign(new Error("no"), { name: "NotAllowedError" })),
      newId: () => "0011223344556677",
      setTimeout: () => 0,
      clearTimeout: () => {},
      onMicDenied: () => denied++,
    });
    await expect(s.start()).rejects.toThrow();
    expect(denied).toBe(1);
    expect(sent).toEqual(["voice_start", "voice_cancel"]);
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
  it("says why nothing will come of a request: no connection, a broken microphone, a refusal of the PC (T32.H.3)", async () => {
    const said: string[] = [];
    const extra: Partial<VoiceDeps> = { onFailed: (r) => said.push(r) };
    await expect(rig({ online: false, extra }).s.start()).rejects.toThrow("offline");
    await expect(rig({ micFails: true, extra }).s.start()).rejects.toThrow();
    expect(said).toEqual(["voice.reply.offline_pc", "voice.reply.mic_unavailable"]);

    said.length = 0;
    const r = rig({ extra });
    await open(r, "hold");
    r.s.refused("RATE_LIMITED", "another voice request is running");
    // The chunks already on the way are refused too: only the first reason is told.
    r.s.refused("BAD_MESSAGE", "bad voice chunk: no such recording");
    expect(said).toEqual(["voice.reply.busy"]);
    expect(r.s.active).toBe(false);
    expect(r.stopped()).toBe(1);

    for (const [code, message, key] of [["NOT_FOUND", "voice control is off", "voice.reply.off_on_pc"], ["BAD_MESSAGE", "voice needs the encrypted channel", "voice.reply.pc_refused"]] as const) {
      said.length = 0;
      const again = rig({ extra });
      await open(again, "tap");
      // A short recording may be over before the refusal arrives.
      again.s.stop();
      again.s.refused(code, message);
      expect(said).toEqual([key]);
    }
    // Without a request there is nothing to explain.
    said.length = 0;
    rig({ extra }).s.refused("NOT_FOUND", "voice control is off");
    expect(said).toEqual([]);
  });

  it("a channel lost in the middle says so", async () => {
    const said: string[] = [];
    const r = rig({ extra: { onFailed: (k) => said.push(k) } });
    await open(r, "hold");
    r.offline();
    r.feed(loud());
    expect(said).toEqual(["voice.reply.offline_pc"]);
    expect(r.s.active).toBe(false);
  });
});
