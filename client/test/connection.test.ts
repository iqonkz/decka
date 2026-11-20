import { afterEach, beforeEach, describe, expect, test, vi } from "vitest";
import type { PatchMsg, WelcomeMsg } from "@decka/shared";
import welcomeFixture from "@decka/shared/fixtures/s2c-welcome.json";
import { buildCandidates } from "../src/net/candidates";
import { Connection, type ConnFailure, type FatalCode, type WebSocketLike } from "../src/net/connection";
import { Opener, Sealer, deriveKeys, fromB64u, kDev, mac, randomBytes, sCode, toB64u } from "../src/net/secure";

const welcome = welcomeFixture as unknown as WelcomeMsg;

const TOKEN = "t".repeat(43);
const enc = new TextEncoder();
const dec = new TextDecoder();

/** A fake socket whose "server" side speaks protocol v2 (PROTOCOL.md 10) with the real crypto module. */
class FakeWS implements WebSocketLike {
  static instances: FakeWS[] = [];
  onopen: (() => void) | null = null;
  onmessage: ((ev: { data: unknown }) => void) | null = null;
  onclose: (() => void) | null = null;
  onerror: (() => void) | null = null;
  binaryType?: string;
  /** Plain text frames of the client (the handshake). */
  plain: Record<string, unknown>[] = [];
  /** Decrypted messages of the client after the handshake. */
  app: Record<string, unknown>[] = [];
  /** Every frame the client sent as text after the handshake must not exist. */
  textAfterAuth = 0;
  closed = false;
  tx: Sealer | null = null;
  rx: Opener | null = null;
  constructor(public url: string) {
    FakeWS.instances.push(this);
  }
  send(data: string | Uint8Array): void {
    if (typeof data === "string") {
      if (this.rx) this.textAfterAuth++;
      this.plain.push(JSON.parse(data) as Record<string, unknown>);
    } else {
      const plain = this.rx?.open(data);
      if (plain) this.app.push(JSON.parse(dec.decode(plain)) as Record<string, unknown>);
    }
  }
  close(): void {
    this.closed = true;
  }
  // test helpers
  serverOpen(): void {
    this.onopen?.();
  }
  /** Plain before the handshake ends, encrypted afterwards. */
  serverSend(obj: unknown): void {
    if (this.tx) this.onmessage?.({ data: this.tx.seal(enc.encode(JSON.stringify(obj))).buffer });
    else this.onmessage?.({ data: JSON.stringify(obj) });
  }
  serverSendFrame(frame: Uint8Array): void {
    this.onmessage?.({ data: frame.buffer.slice(frame.byteOffset, frame.byteOffset + frame.byteLength) });
  }
  serverClose(): void {
    this.onclose?.();
  }
  /** The messages the app sent (decrypted), in order. */
  sentMsgs(): Record<string, unknown>[] {
    return this.app;
  }
  /** The server side of the handshake: answers `hello2` with `challenge`, checks `auth`, sends `auth_ok`. */
  serverHandshake(ikmFor: (cn: Uint8Array, sn: Uint8Array) => Uint8Array, opts: { forge?: boolean } = {}): void {
    const hello = this.plain[0] as { cn: string };
    const cn = fromB64u(hello.cn) as Uint8Array;
    const sn = randomBytes(16);
    this.serverSend({ t: "challenge", sn: toB64u(sn), agent: { name: "PC", version: "0.9.0" } });
    const auth = this.plain.find((m) => m.t === "auth") as { mac: string } | undefined;
    const keys = deriveKeys(ikmFor(cn, sn), cn, sn);
    if (!auth || toB64u(mac(keys.auth, "C", cn, sn)) !== auth.mac) throw new Error("the client proof does not match");
    const serverKeys = opts.forge ? deriveKeys(new Uint8Array(32).fill(9), cn, sn) : keys;
    this.serverSend({ t: "auth_ok", mac: toB64u(mac(serverKeys.auth, "S", cn, sn)) });
    if (opts.forge) return;
    this.tx = new Sealer(keys.s2c);
    this.rx = new Opener(keys.c2s);
  }
}

interface Setup {
  conn: Connection;
  statuses: string[];
  fatals: FatalCode[];
  patches: PatchMsg[];
}

function setup(opts: { candidates?: string[]; native?: boolean; token?: string | null } = {}): Setup {
  const statuses: string[] = [];
  const fatals: FatalCode[] = [];
  const patches: PatchMsg[] = [];
  const conn = new Connection({
    WebSocketCtor: FakeWS,
    now: () => Date.now(),
    setTimeout: (fn, ms) => setTimeout(fn, ms),
    clearTimeout: (h) => clearTimeout(h as ReturnType<typeof setTimeout>),
    candidates: () => opts.candidates ?? ["pc:9247"],
    token: () => (opts.token === undefined ? TOKEN : opts.token),
    deviceId: () => "d-1",
    native: opts.native ?? false,
    clientInfo: { name: "test", app: "web", version: "0.0.0" },
    onWelcome: () => {},
    onPatch: (m) => patches.push(m),
    onPressResult: () => {},
    onStatus: (c, via) => statuses.push(`${c}:${via ?? "-"}`),
    onFatal: (c) => fatals.push(c),
  });
  return { conn, statuses, fatals, patches };
}

const last = () => FakeWS.instances[FakeWS.instances.length - 1] as FakeWS;

/** Opens the newest socket, runs the device handshake and answers with a welcome. */
function goOnline(): FakeWS {
  const ws = last();
  ws.serverOpen();
  ws.serverHandshake(() => kDev(TOKEN));
  ws.serverSend(welcome);
  return ws;
}

beforeEach(() => {
  FakeWS.instances = [];
  vi.useFakeTimers();
});
afterEach(() => vi.useRealTimers());

describe("buildCandidates", () => {
  test("native: usb first, then saved hosts, no duplicates", () => {
    expect(buildCandidates({ native: true }, { hosts: ["10.0.0.5", "127.0.0.1", "10.0.0.5"], port: 9000 })).toEqual([
      "127.0.0.1:9000",
      "10.0.0.5:9000",
    ]);
    expect(buildCandidates({ native: true }, null)).toEqual(["127.0.0.1:9247"]);
  });
  test("web: dev host, else location host", () => {
    expect(buildCandidates({ native: false, devHost: "pc:1", locationHost: "x:2" }, null)).toEqual(["pc:1"]);
    expect(buildCandidates({ native: false, locationHost: "x:2" }, null)).toEqual(["x:2"]);
  });
});

describe("Connection", () => {
  test("sends hello2 as a device after open; the token itself never travels", () => {
    const { conn } = setup();
    conn.start();
    last().serverOpen();
    const [hello] = last().plain;
    expect(hello).toMatchObject({ t: "hello2", v: 2, mode: "device", deviceId: "d-1" });
    expect(JSON.stringify(hello)).not.toContain(TOKEN);
  });

  test("the client proof is sent after the challenge and everything after auth_ok is encrypted", () => {
    const { conn } = setup();
    conn.start();
    const ws = goOnline();
    expect(ws.plain.map((m) => m.t)).toEqual(["hello2", "auth"]);
    vi.advanceTimersByTime(10_000);
    expect(ws.sentMsgs().map((m) => m.t)).toEqual(["ping"]);
    expect(ws.textAfterAuth).toBe(0);
  });

  test("the client never speaks protocol v1", () => {
    const { conn } = setup();
    conn.start();
    const ws = goOnline();
    // Even an agent that complains about the version cannot make the client fall back.
    ws.serverSend({ t: "error", code: "PROTOCOL_VERSION", message: "x", fatal: true, supported: [1] });
    vi.advanceTimersByTime(120_000);
    const sent = FakeWS.instances.flatMap((w) => [...w.plain, ...w.app]).map((m) => m.t);
    expect(sent).not.toContain("hello");
    expect(sent).not.toContain("pair");
  });

  test("a forged auth_ok ends the connection with SERVER_MISMATCH and nothing retries", () => {
    const { conn, fatals } = setup();
    conn.start();
    const ws = last();
    ws.serverOpen();
    ws.serverHandshake(() => kDev(TOKEN), { forge: true });
    expect(fatals).toEqual(["SERVER_MISMATCH"]);
    expect(ws.closed).toBe(true);
    vi.advanceTimersByTime(120_000);
    expect(FakeWS.instances).toHaveLength(1);
  });

  test("a replayed, reordered or changed server frame drops the link", () => {
    for (const attack of ["replay", "gap", "tamper"] as const) {
      FakeWS.instances = [];
      const { conn, patches } = setup();
      conn.start();
      const ws = goOnline();
      const tx = ws.tx as Sealer;
      const frame = (n: number) => tx.seal(enc.encode(JSON.stringify({ t: "patch", rev: n, section: "vars", value: {} })));
      const f1 = frame(1);
      ws.serverSendFrame(f1);
      expect(patches, attack).toHaveLength(1);
      if (attack === "replay") ws.serverSendFrame(f1);
      if (attack === "gap") {
        frame(2); // skipped on the wire
        ws.serverSendFrame(frame(3));
      }
      if (attack === "tamper") {
        const f = frame(2);
        f[f.length - 1] = (f[f.length - 1] as number) ^ 1;
        ws.serverSendFrame(f);
      }
      expect(patches, attack).toHaveLength(1);
      expect(ws.closed, attack).toBe(true);
      conn.stop();
    }
  });

  test("a silent first candidate is abandoned after 1500 ms", () => {
    const { conn } = setup({ candidates: ["a:1", "b:2"] });
    conn.start();
    expect(FakeWS.instances.map((w) => w.url)).toEqual(["ws://a:1/ws"]);
    last().serverOpen();
    vi.advanceTimersByTime(1499);
    expect(FakeWS.instances).toHaveLength(1);
    vi.advanceTimersByTime(1);
    expect(FakeWS.instances[0]?.closed).toBe(true);
    expect(FakeWS.instances.map((w) => w.url)).toEqual(["ws://a:1/ws", "ws://b:2/ws"]);
  });

  test("via is usb only for the loopback candidate in native mode", () => {
    const a = setup({ candidates: ["127.0.0.1:9247"], native: true });
    a.conn.start();
    goOnline();
    expect(a.statuses).toEqual(["connecting:-", "online:usb"]);
    a.conn.stop();

    const b = setup({ candidates: ["127.0.0.1:9247"], native: false });
    b.conn.start();
    goOnline();
    expect(b.statuses).toEqual(["connecting:-", "online:lan"]);
  });

  test("retry delays are 1, 2, 4, 8, 15, 15 seconds and reset after a welcome", () => {
    const { conn, statuses } = setup();
    conn.start();
    const delays = [1, 2, 4, 8, 15, 15];
    for (const [i, s] of delays.entries()) {
      expect(FakeWS.instances).toHaveLength(i + 1);
      last().serverClose();
      vi.advanceTimersByTime(s * 1000 - 1);
      expect(FakeWS.instances, `before ${s}s`).toHaveLength(i + 1);
      vi.advanceTimersByTime(1);
    }
    expect(statuses).toContain("offline:-");

    goOnline(); // success resets the series
    last().serverClose(); // connection lost after being online
    vi.advanceTimersByTime(999);
    const count = FakeWS.instances.length;
    vi.advanceTimersByTime(1);
    expect(FakeWS.instances).toHaveLength(count + 1);
  });

  test("press results are matched by id even when they arrive out of order", async () => {
    const { conn } = setup();
    conn.start();
    const ws = goOnline();
    const first = conn.press("a");
    const second = conn.press("b");
    const sent = ws.sentMsgs().filter((m) => m.t === "press");
    expect(sent).toEqual([
      { t: "press", id: 1, actionId: "a" },
      { t: "press", id: 2, actionId: "b" },
    ]);
    ws.serverSend({ t: "press_result", id: 2, ok: false, error: { code: "ACTION_FAILED", message: "boom" } });
    ws.serverSend({ t: "press_result", id: 1, ok: true });
    await expect(first).resolves.toBeUndefined();
    await expect(second).rejects.toMatchObject({ code: "ACTION_FAILED", message: "boom" });
  });

  test("a press without an answer is rejected after 5 s", async () => {
    const { conn } = setup();
    conn.start();
    goOnline();
    const p = conn.press("a");
    const result = p.catch((e: ConnFailure) => e);
    vi.advanceTimersByTime(5000);
    expect(await result).toEqual({ code: "TIMEOUT" });
  });

  test("press while offline is rejected with OFFLINE", async () => {
    const { conn } = setup();
    await expect(conn.press("a")).rejects.toEqual({ code: "OFFLINE" });
  });

  const pressFrames = (ws: FakeWS) => ws.sentMsgs().filter((m) => m.t === "press");

  test("a press made while offline is rejected and never sent after reconnecting (S15)", async () => {
    const { conn } = setup();
    conn.start();
    const first = goOnline();
    first.serverClose(); // the link drops
    await expect(conn.press("a")).rejects.toEqual({ code: "OFFLINE" });

    vi.advanceTimersByTime(1000); // reconnect attempt
    const second = goOnline();
    vi.advanceTimersByTime(30_000);
    expect(pressFrames(first)).toEqual([]);
    expect(pressFrames(second)).toEqual([]);
    // Only the handshake and heartbeat pings travel on the new socket.
    expect(second.plain.map((m) => m.t)).toEqual(["hello2", "auth"]);
    expect(second.sentMsgs().map((m) => m.t).filter((t) => t !== "ping")).toEqual([]);
  });

  test("a press in flight when the link drops is rejected and not resent (S15)", async () => {
    const { conn } = setup();
    conn.start();
    const first = goOnline();
    const inFlight = conn.press("a").catch((e: ConnFailure) => e);
    expect(pressFrames(first)).toHaveLength(1);
    first.serverClose();
    expect(await inFlight).toEqual({ code: "OFFLINE" });

    vi.advanceTimersByTime(1000);
    const second = goOnline();
    vi.advanceTimersByTime(30_000);
    expect(pressFrames(first)).toHaveLength(1); // still the single original frame
    expect(pressFrames(second)).toEqual([]);
  });

  test("UNAUTHORIZED is fatal and stops reconnecting", () => {
    const { conn, fatals } = setup();
    conn.start();
    const ws = last();
    ws.serverOpen();
    ws.serverSend({ t: "error", code: "UNAUTHORIZED", message: "x", fatal: true });
    expect(fatals).toEqual(["UNAUTHORIZED"]);
    ws.serverClose();
    vi.advanceTimersByTime(120_000);
    expect(FakeWS.instances).toHaveLength(1);
  });

  test("25 s of silence makes the client close the socket", () => {
    const { conn } = setup();
    conn.start();
    const ws = goOnline();
    vi.advanceTimersByTime(24_999);
    expect(ws.closed).toBe(false);
    vi.advanceTimersByTime(1);
    expect(ws.closed).toBe(true);
  });

  test("incoming frames keep the connection alive and pings go out every 10 s", () => {
    const { conn } = setup();
    conn.start();
    const ws = goOnline();
    for (let i = 0; i < 5; i++) {
      vi.advanceTimersByTime(10_000);
      ws.serverSend({ t: "pong", ts: 1 });
    }
    expect(ws.closed).toBe(false);
    expect(ws.sentMsgs().filter((m) => m.t === "ping")).toHaveLength(5);
  });

  test("patches are forwarded", () => {
    const { conn, patches } = setup();
    conn.start();
    const ws = goOnline();
    ws.serverSend({ t: "patch", rev: 43, section: "vars", value: {} });
    expect(patches).toHaveLength(1);
  });

  test("pair by the typed code runs pair_code and ends on paired", async () => {
    const { conn } = setup();
    const p = conn.pair("TESTCDE2");
    const ws = last();
    ws.serverOpen();
    expect(ws.plain[0]).toMatchObject({ t: "hello2", v: 2, mode: "pair_code" });
    expect(ws.plain[0]).not.toHaveProperty("deviceId");
    ws.serverHandshake((cn, sn) => sCode("TESTCDE2", cn, sn));
    ws.serverSend({ t: "paired", deviceId: "d-1", token: "x".repeat(43) });
    await expect(p).resolves.toMatchObject({ deviceId: "d-1" });
    expect(ws.closed).toBe(true); // the caller saves the token and reconnects as a device
  });

  test("pair with the secret of the QR runs pair_qr", async () => {
    const { conn } = setup();
    const secret = new Uint8Array(32).map((_, i) => i * 3);
    const p = conn.pair("TESTCDE2", toB64u(secret));
    const ws = last();
    ws.serverOpen();
    expect(ws.plain[0]).toMatchObject({ t: "hello2", mode: "pair_qr" });
    ws.serverHandshake(() => secret);
    ws.serverSend({ t: "paired", deviceId: "d-2", token: "y".repeat(43) });
    await expect(p).resolves.toMatchObject({ deviceId: "d-2" });
  });

  test("a PC that cannot prove the QR secret is refused while pairing", async () => {
    const { conn } = setup();
    const p = conn.pair("TESTCDE2", toB64u(new Uint8Array(32).fill(5)));
    const result = p.catch((e: ConnFailure) => e);
    const ws = last();
    ws.serverOpen();
    ws.serverHandshake(() => new Uint8Array(32).fill(5), { forge: true });
    expect(await result).toEqual({ code: "SERVER_MISMATCH" });
  });

  test("a wrong code rejects the pairing with its code", async () => {
    const { conn } = setup();
    const p = conn.pair("WRONGCDE");
    const result = p.catch((e: ConnFailure) => e);
    last().serverOpen();
    last().serverSend({ t: "error", code: "PAIR_CODE_INVALID", message: "x", fatal: true });
    expect(await result).toEqual({ code: "PAIR_CODE_INVALID" });
  });

  test("pairing against an unreachable PC rejects with UNREACHABLE", async () => {
    const { conn } = setup();
    const p = conn.pair("TESTCDE2");
    const result = p.catch((e: ConnFailure) => e);
    last().serverClose();
    expect(await result).toEqual({ code: "UNREACHABLE" });
  });
});
