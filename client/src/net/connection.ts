import type { ClientInfo, ErrorCode, PairedMsg, PatchMsg, PressResultMsg, ServerMsg, WelcomeMsg } from "@decka/shared";
import { PROTOCOL_VERSION } from "@decka/shared";
import { Opener, Sealer, deriveKeys, fromB64u, kDev, mac, macEquals, randomBytes, sCode, toB64u, type Keys } from "./secure";

/** The subset of WebSocket that Connection uses (also implemented by test fakes). */
export interface WebSocketLike {
  onopen: (() => void) | null;
  onmessage: ((ev: { data: unknown }) => void) | null;
  onclose: (() => void) | null;
  onerror: (() => void) | null;
  binaryType?: string;
  send(data: string | Uint8Array): void;
  close(): void;
}
export type WebSocketCtor = new (url: string) => WebSocketLike;

export type ConnStatus = "connecting" | "online" | "offline";

/** What a rejected press/pair promise carries. */
export interface ConnFailure {
  code: ErrorCode | "TIMEOUT" | "OFFLINE" | "UNREACHABLE" | "SERVER_MISMATCH";
  message?: string;
}

/** The PC did not prove it holds our key (PROTOCOL.md 10): not the PC we paired with. */
export type FatalCode = ErrorCode | "SERVER_MISMATCH";

export interface ConnectionDeps {
  WebSocketCtor: WebSocketCtor;
  now: () => number;
  setTimeout: (fn: () => void, ms: number) => unknown;
  clearTimeout: (handle: unknown) => void;
  candidates: () => string[];
  token: () => string | null;
  deviceId: () => string | null;
  native: boolean;
  clientInfo: ClientInfo;
  onWelcome: (msg: WelcomeMsg) => void;
  onPatch: (msg: PatchMsg) => void;
  onPressResult: (msg: PressResultMsg) => void;
  onStatus: (conn: ConnStatus, via: "usb" | "lan" | null) => void;
  onFatal: (code: FatalCode) => void;
}

/** Handshake state of the open socket (one per attempt). */
interface Handshake {
  mode: "device" | "pair_qr" | "pair_code";
  cn: Uint8Array;
  /** Set once `auth` is sent: the keys and the sn that the server's `auth_ok` is checked with. */
  sent: { keys: Keys; sn: Uint8Array } | null;
}

const ATTEMPT_MS = 1500;
const PING_MS = 10_000;
const SILENCE_MS = 25_000;
const PRESS_TIMEOUT_MS = 5000;
const RETRY_SECONDS = [1, 2, 4, 8, 15];
const FATAL: readonly ErrorCode[] = ["UNAUTHORIZED", "DEVICE_REVOKED", "PROTOCOL_VERSION"];

interface PendingPress {
  resolve: () => void;
  reject: (e: ConnFailure) => void;
  timer: unknown;
}

export class Connection {
  private running = false;
  private ws: WebSocketLike | null = null;
  private welcomed = false;
  private list: string[] = [];
  private idx = 0;
  private delayIdx = 0;
  private status: ConnStatus | null = null;
  private attemptTimer: unknown = null;
  private retryTimer: unknown = null;
  private pingTimer: unknown = null;
  private watchdog: unknown = null;
  private pairCode: string | null = null;
  private pairSecret: Uint8Array | null = null;
  private hs: Handshake | null = null;
  private sec: { tx: Sealer; rx: Opener } | null = null;
  private pairing: { resolve: (m: PairedMsg) => void; reject: (e: ConnFailure) => void } | null = null;
  private pending = new Map<number, PendingPress>();
  private nextId = 0;

  constructor(private readonly d: ConnectionDeps) {}

  start(): void {
    if (this.running) return;
    this.running = true;
    this.delayIdx = 0;
    this.setStatus("connecting", null);
    this.startCycle();
  }

  stop(): void {
    this.running = false;
    this.pairCode = null;
    this.pairSecret = null;
    this.clearTimers();
    this.detach();
    this.failPending({ code: "OFFLINE" });
    this.rejectPairing({ code: "OFFLINE" });
  }

  /**
   * Pairs using the candidates currently returned by `deps.candidates()`. With `secret` (`S_qr` from the QR, base64url)
   * the handshake runs in `pair_qr` mode, otherwise in `pair_code` mode with the typed code. The connection stops once
   * `paired` has arrived; the caller saves the token and calls `start()`.
   */
  pair(code: string, secret?: string): Promise<PairedMsg> {
    this.stop();
    return new Promise<PairedMsg>((resolve, reject) => {
      const s = secret === undefined ? null : fromB64u(secret);
      if (secret !== undefined && (s === null || s.length !== 32)) return reject({ code: "BAD_MESSAGE" } satisfies ConnFailure);
      this.pairing = { resolve, reject };
      this.pairCode = code;
      this.pairSecret = s;
      this.running = true;
      this.delayIdx = 0;
      this.setStatus("connecting", null);
      this.startCycle();
    });
  }

  press(actionId: string): Promise<void> {
    const ws = this.ws;
    if (!ws || !this.welcomed) return Promise.reject<void>({ code: "OFFLINE" } satisfies ConnFailure);
    const id = ++this.nextId;
    return new Promise<void>((resolve, reject) => {
      const timer = this.d.setTimeout(() => {
        this.pending.delete(id);
        reject({ code: "TIMEOUT" } satisfies ConnFailure);
      }, PRESS_TIMEOUT_MS);
      this.pending.set(id, { resolve, reject, timer });
      // The phone sends only the action id: exactly these three fields.
      this.sendMsg({ t: "press", id, actionId });
    });
  }

  dismiss(notifId: string): void {
    if (this.ws && this.welcomed) this.sendMsg({ t: "notif_dismiss", notifId });
  }

  /** Asks the PC to open the program a Windows notification came from (DATA_MODEL.md 10.4); only the id is sent. */
  openNotification(notifId: string): void {
    if (this.ws && this.welcomed) this.sendMsg({ t: "notif_open", notifId });
  }

  /** The answer to a permission question of Claude Code (DATA_MODEL.md 6.8). */
  permAnswer(reqId: string, decision: "allow" | "allow_always" | "deny" | "pc" | "reply", reply?: number): void {
    if (this.ws && this.welcomed) this.sendMsg({ t: "perm_answer", reqId, decision, ...(reply !== undefined ? { reply } : {}) });
  }

  /** After the handshake every message goes out as an encrypted binary frame; never as plain text. */
  private sendMsg(msg: Record<string, unknown>): void {
    const ws = this.ws;
    if (!ws || !this.sec) return;
    ws.send(this.sec.tx.seal(new TextEncoder().encode(JSON.stringify(msg))));
  }

  /** The page became visible: if there is no live socket, retry right now. */
  wake(): void {
    if (!this.running || this.ws || this.pairCode !== null) return;
    if (this.retryTimer !== null) {
      this.d.clearTimeout(this.retryTimer);
      this.retryTimer = null;
    }
    this.startCycle();
  }

  // ----- connection cycle -----

  private startCycle(): void {
    this.list = this.d.candidates();
    this.idx = 0;
    this.openCandidate();
  }

  private openCandidate(): void {
    if (!this.running) return;
    const host = this.list[this.idx];
    if (host === undefined) return this.cycleFailed();
    let ws: WebSocketLike;
    try {
      ws = new this.d.WebSocketCtor(`ws://${host}/ws`);
    } catch {
      this.idx++;
      return this.openCandidate();
    }
    this.ws = ws;
    this.welcomed = false;
    this.nextId = 0;
    this.hs = null;
    this.sec = null;
    ws.binaryType = "arraybuffer";
    ws.onopen = () => {
      if (this.ws === ws) this.sendFirst(ws);
    };
    ws.onmessage = (ev) => {
      if (this.ws === ws) this.handleFrame(ev.data);
    };
    ws.onclose = ws.onerror = () => {
      if (this.ws === ws) this.socketClosed();
    };
    this.attemptTimer = this.d.setTimeout(() => {
      this.attemptTimer = null;
      if (this.ws === ws && !this.welcomed) {
        this.detach();
        this.idx++;
        this.openCandidate();
      }
    }, ATTEMPT_MS);
  }

  /** Step 1 of the v2 handshake. This client never falls back to protocol v1. */
  private sendFirst(ws: WebSocketLike): void {
    const cn = randomBytes(16);
    let hello: Record<string, unknown>;
    if (this.pairCode !== null) {
      const mode = this.pairSecret !== null ? "pair_qr" : "pair_code";
      this.hs = { mode, cn, sent: null };
      hello = { t: "hello2", v: PROTOCOL_VERSION, mode, cn: toB64u(cn), client: this.d.clientInfo };
    } else {
      const deviceId = this.d.deviceId();
      if (this.d.token() === null || deviceId === null) {
        this.stop();
        this.d.onFatal("UNAUTHORIZED");
        return;
      }
      this.hs = { mode: "device", cn, sent: null };
      hello = { t: "hello2", v: PROTOCOL_VERSION, mode: "device", deviceId, cn: toB64u(cn), client: this.d.clientInfo };
    }
    ws.send(JSON.stringify(hello));
  }

  /** Steps 2-3: the `challenge` arrived; derive the keys and prove we hold the secret. */
  private onChallenge(sn: Uint8Array): void {
    const hs = this.hs;
    const ws = this.ws;
    if (!hs || !ws || hs.sent) return;
    this.clearAttempt(); // the right PC answered; PBKDF2 may take a while on a slow phone
    let ikm: Uint8Array;
    if (hs.mode === "device") ikm = kDev(this.d.token() ?? "");
    else if (hs.mode === "pair_qr") ikm = this.pairSecret ?? new Uint8Array(32);
    else ikm = sCode(this.pairCode ?? "", hs.cn, sn);
    const keys = deriveKeys(ikm, hs.cn, sn);
    hs.sent = { keys, sn };
    ws.send(JSON.stringify({ t: "auth", mac: toB64u(mac(keys.auth, "C", hs.cn, sn)) }));
  }

  /** Step 5: the PC must prove the same; a forged `auth_ok` ends the connection for good (until a manual retry). */
  private onAuthOk(received: Uint8Array | null): void {
    const hs = this.hs;
    if (!hs?.sent) return;
    const expected = mac(hs.sent.keys.auth, "S", hs.cn, hs.sent.sn);
    if (received === null || !macEquals(expected, received)) {
      this.rejectPairing({ code: "SERVER_MISMATCH" });
      this.stop();
      this.d.onFatal("SERVER_MISMATCH");
      return;
    }
    this.sec = { tx: new Sealer(hs.sent.keys.c2s), rx: new Opener(hs.sent.keys.s2c) };
    this.hs = null;
  }

  private cycleFailed(): void {
    if (this.pairCode !== null) {
      this.rejectPairing({ code: "UNREACHABLE" });
      this.stop();
      return;
    }
    this.setStatus("offline", null);
    this.scheduleRetry();
  }

  private scheduleRetry(): void {
    const seconds = RETRY_SECONDS[Math.min(this.delayIdx, RETRY_SECONDS.length - 1)] ?? 15;
    this.delayIdx++;
    this.retryTimer = this.d.setTimeout(() => {
      this.retryTimer = null;
      if (this.running) this.startCycle();
    }, seconds * 1000);
  }

  private socketClosed(): void {
    const wasWelcomed = this.welcomed;
    this.detach();
    if (!wasWelcomed) {
      this.idx++;
      this.openCandidate();
    } else {
      this.connectionLost();
    }
  }

  private connectionLost(): void {
    this.failPending({ code: "OFFLINE" });
    this.setStatus("offline", null);
    this.scheduleRetry();
  }

  // ----- incoming frames -----

  private handleFrame(data: unknown): void {
    let text: string;
    if (this.sec) {
      // After the handshake only encrypted binary frames are accepted; any failure of the checks ends the link.
      const bytes =
        data instanceof ArrayBuffer ? new Uint8Array(data) : ArrayBuffer.isView(data) ? new Uint8Array(data.buffer, data.byteOffset, data.byteLength) : null;
      const plain = bytes ? this.sec.rx.open(bytes) : null;
      if (plain === null) return this.socketClosed();
      text = new TextDecoder().decode(plain);
    } else {
      if (typeof data !== "string") return; // no binary frame is expected before auth_ok
      text = data;
    }
    let msg: ServerMsg;
    try {
      msg = JSON.parse(text) as ServerMsg;
    } catch {
      return;
    }
    if (typeof msg !== "object" || msg === null) return;

    if (!this.sec) {
      // Handshake phase: only these three messages mean anything.
      if (msg.t === "challenge") this.onChallenge(fromB64u(msg.sn) ?? new Uint8Array(0));
      else if (msg.t === "auth_ok") this.onAuthOk(fromB64u(msg.mac));
      else if (msg.t === "error") this.onError(msg.code);
      return;
    }
    if (this.welcomed) this.resetWatchdog();

    switch (msg.t) {
      case "paired":
        // Pairing is done: the connection ends and the caller reconnects as a device with the saved token.
        this.pairing?.resolve(msg);
        this.pairing = null;
        this.stop();
        break;
      case "welcome":
        this.onWelcomeMsg(msg);
        break;
      case "patch":
        this.d.onPatch(msg);
        break;
      case "press_result":
        this.onPressResult(msg);
        break;
      case "error":
        this.onError(msg.code);
        break;
      default:
        break; // pong and unknown types
    }
  }

  private onWelcomeMsg(msg: WelcomeMsg): void {
    this.clearAttempt();
    this.welcomed = true;
    this.delayIdx = 0;
    const via = this.d.native && (this.list[this.idx] ?? "").startsWith("127.0.0.1:") ? "usb" : "lan";
    this.setStatus("online", via);
    this.resetWatchdog();
    this.schedulePing();
    this.d.onWelcome(msg);
  }

  private onPressResult(msg: PressResultMsg): void {
    this.d.onPressResult(msg);
    const p = this.pending.get(msg.id);
    if (!p) return;
    this.pending.delete(msg.id);
    this.d.clearTimeout(p.timer);
    if (msg.ok) p.resolve();
    else p.reject({ code: msg.error?.code ?? "INTERNAL", message: msg.error?.message ?? "" });
  }

  private onError(code: ErrorCode): void {
    if (this.pairCode !== null) {
      // While pairing, any refusal ends the attempt; the caller decides what to show.
      this.rejectPairing({ code });
      this.stop();
      return;
    }
    if (FATAL.includes(code)) {
      this.stop();
      this.d.onFatal(code);
    }
  }

  // ----- heartbeat -----

  private schedulePing(): void {
    this.pingTimer = this.d.setTimeout(() => {
      this.sendMsg({ t: "ping", ts: this.d.now() });
      this.schedulePing();
    }, PING_MS);
  }

  private resetWatchdog(): void {
    if (this.watchdog !== null) this.d.clearTimeout(this.watchdog);
    this.watchdog = this.d.setTimeout(() => {
      this.watchdog = null;
      this.clearHeartbeat();
      this.detach();
      this.connectionLost();
    }, SILENCE_MS);
  }

  // ----- helpers -----

  private setStatus(conn: ConnStatus, via: "usb" | "lan" | null): void {
    if (this.status === conn && conn !== "online") return;
    this.status = conn;
    this.d.onStatus(conn, via);
  }

  private clearAttempt(): void {
    if (this.attemptTimer !== null) {
      this.d.clearTimeout(this.attemptTimer);
      this.attemptTimer = null;
    }
  }

  private clearHeartbeat(): void {
    if (this.pingTimer !== null) this.d.clearTimeout(this.pingTimer);
    if (this.watchdog !== null) this.d.clearTimeout(this.watchdog);
    this.pingTimer = null;
    this.watchdog = null;
  }

  private clearTimers(): void {
    this.clearAttempt();
    this.clearHeartbeat();
    if (this.retryTimer !== null) this.d.clearTimeout(this.retryTimer);
    this.retryTimer = null;
  }

  /** Drops the current socket without reacting to its events. */
  private detach(): void {
    const ws = this.ws;
    this.ws = null;
    this.welcomed = false;
    this.hs = null;
    this.sec = null;
    this.clearAttempt();
    this.clearHeartbeat();
    if (!ws) return;
    ws.onopen = ws.onmessage = ws.onclose = ws.onerror = null;
    try {
      ws.close();
    } catch {
      // already closed
    }
  }

  private failPending(e: ConnFailure): void {
    for (const p of this.pending.values()) {
      this.d.clearTimeout(p.timer);
      p.reject(e);
    }
    this.pending.clear();
  }

  private rejectPairing(e: ConnFailure): void {
    this.pairing?.reject(e);
    this.pairing = null;
  }
}
