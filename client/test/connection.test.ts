import { afterEach, beforeEach, describe, expect, test, vi } from "vitest";
import type { ErrorCode, PatchMsg, WelcomeMsg } from "@decka/shared";
import welcomeFixture from "@decka/shared/fixtures/s2c-welcome.json";
import { buildCandidates } from "../src/net/candidates";
import { Connection, type ConnFailure, type WebSocketLike } from "../src/net/connection";

const welcome = welcomeFixture as unknown as WelcomeMsg;

class FakeWS implements WebSocketLike {
  static instances: FakeWS[] = [];
  onopen: (() => void) | null = null;
  onmessage: ((ev: { data: unknown }) => void) | null = null;
  onclose: (() => void) | null = null;
  onerror: (() => void) | null = null;
  sent: string[] = [];
  closed = false;
  constructor(public url: string) {
    FakeWS.instances.push(this);
  }
  send(data: string): void {
    this.sent.push(data);
  }
  close(): void {
    this.closed = true;
  }
  // test helpers
  serverOpen(): void {
    this.onopen?.();
  }
  serverSend(obj: unknown): void {
    this.onmessage?.({ data: JSON.stringify(obj) });
  }
  serverClose(): void {
    this.onclose?.();
  }
  sentMsgs(): Record<string, unknown>[] {
    return this.sent.map((s) => JSON.parse(s) as Record<string, unknown>);
  }
}

interface Setup {
  conn: Connection;
  statuses: string[];
  fatals: ErrorCode[];
  patches: PatchMsg[];
}

function setup(opts: { candidates?: string[]; native?: boolean; token?: string | null } = {}): Setup {
  const statuses: string[] = [];
  const fatals: ErrorCode[] = [];
  const patches: PatchMsg[] = [];
  const conn = new Connection({
    WebSocketCtor: FakeWS,
    now: () => Date.now(),
    setTimeout: (fn, ms) => setTimeout(fn, ms),
    clearTimeout: (h) => clearTimeout(h as ReturnType<typeof setTimeout>),
    candidates: () => opts.candidates ?? ["pc:9247"],
    token: () => (opts.token === undefined ? "t".repeat(43) : opts.token),
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

/** Opens the newest socket and answers its hello with a welcome. */
function goOnline(): FakeWS {
  const ws = last();
  ws.serverOpen();
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
  test("sends hello with the saved token after open", () => {
    const { conn } = setup();
    conn.start();
    last().serverOpen();
    const [hello] = last().sentMsgs();
    expect(hello).toMatchObject({ t: "hello", v: 1, token: "t".repeat(43) });
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

  test("pair resolves on paired and keeps the socket for the welcome", async () => {
    const { conn } = setup();
    const p = conn.pair("TESTCDE2");
    const ws = last();
    ws.serverOpen();
    expect(ws.sentMsgs()[0]).toMatchObject({ t: "pair", v: 1, code: "TESTCDE2" });
    ws.serverSend({ t: "paired", deviceId: "d-1", token: "x".repeat(43) });
    await expect(p).resolves.toMatchObject({ deviceId: "d-1" });
    ws.serverSend(welcome);
    expect(ws.closed).toBe(false);
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
