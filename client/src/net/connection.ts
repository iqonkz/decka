import type { ClientInfo, ErrorCode, PairedMsg, PatchMsg, PressResultMsg, ServerMsg, WelcomeMsg } from "@decka/shared";
import { PROTOCOL_VERSION } from "@decka/shared";

/** The subset of WebSocket that Connection uses (also implemented by test fakes). */
export interface WebSocketLike {
  onopen: (() => void) | null;
  onmessage: ((ev: { data: unknown }) => void) | null;
  onclose: (() => void) | null;
  onerror: (() => void) | null;
  send(data: string): void;
  close(): void;
}
export type WebSocketCtor = new (url: string) => WebSocketLike;

export type ConnStatus = "connecting" | "online" | "offline";

/** What a rejected press/pair promise carries. */
export interface ConnFailure {
  code: ErrorCode | "TIMEOUT" | "OFFLINE" | "UNREACHABLE";
  message?: string;
}

export interface ConnectionDeps {
  WebSocketCtor: WebSocketCtor;
  now: () => number;
  setTimeout: (fn: () => void, ms: number) => unknown;
  clearTimeout: (handle: unknown) => void;
  candidates: () => string[];
  token: () => string | null;
  native: boolean;
  clientInfo: ClientInfo;
  onWelcome: (msg: WelcomeMsg) => void;
  onPatch: (msg: PatchMsg) => void;
  onPressResult: (msg: PressResultMsg) => void;
  onStatus: (conn: ConnStatus, via: "usb" | "lan" | null) => void;
  onFatal: (code: ErrorCode) => void;
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
    this.clearTimers();
    this.detach();
    this.failPending({ code: "OFFLINE" });
    this.rejectPairing({ code: "OFFLINE" });
  }

  /** Pairs using the candidates currently returned by `deps.candidates()`. */
  pair(code: string): Promise<PairedMsg> {
    this.stop();
    return new Promise<PairedMsg>((resolve, reject) => {
      this.pairing = { resolve, reject };
      this.pairCode = code;
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
      ws.send(JSON.stringify({ t: "press", id, actionId }));
    });
  }

  dismiss(notifId: string): void {
    if (this.ws && this.welcomed) this.ws.send(JSON.stringify({ t: "notif_dismiss", notifId }));
  }

  /** The answer to a permission question of Claude Code (DATA_MODEL.md 6.8). */
  permAnswer(reqId: string, decision: "allow" | "deny" | "pc"): void {
    if (this.ws && this.welcomed) this.ws.send(JSON.stringify({ t: "perm_answer", reqId, decision }));
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

  private sendFirst(ws: WebSocketLike): void {
    if (this.pairCode !== null) {
      ws.send(JSON.stringify({ t: "pair", v: PROTOCOL_VERSION, code: this.pairCode, client: this.d.clientInfo }));
      return;
    }
    const token = this.d.token();
    if (token === null) {
      this.stop();
      this.d.onFatal("UNAUTHORIZED");
      return;
    }
    ws.send(JSON.stringify({ t: "hello", v: PROTOCOL_VERSION, token, client: this.d.clientInfo }));
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
    let msg: ServerMsg;
    try {
      msg = JSON.parse(String(data)) as ServerMsg;
    } catch {
      return;
    }
    if (typeof msg !== "object" || msg === null) return;
    if (this.welcomed) this.resetWatchdog();

    switch (msg.t) {
      case "paired":
        this.pairCode = null; // later reconnects use the saved token
        this.pairing?.resolve(msg);
        this.pairing = null;
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
      this.ws?.send(JSON.stringify({ t: "ping", ts: this.d.now() }));
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
