// The single place where Connection, the store and local storage meet (ARCHITECTURE.md 4.4).
import type { ErrorCode } from "@decka/shared";
import { parsePairUrl } from "@decka/shared";
import { buildCandidates } from "../net/candidates";
import { Connection, type ConnFailure, type WebSocketCtor } from "../net/connection";
import { clientInfo, isNative } from "../platform/native";
import { applyPatch, applyWelcome, setState } from "../store/store";
import { clearConn, loadConn, saveConn } from "../store/storage";

/** Where the next pairing attempt goes; null when not pairing. */
let pairTarget: { hosts: string[]; port: number } | null = null;
let connection: Connection | null = null;

function candidates(): string[] {
  const env = { native: isNative(), devHost: import.meta.env.VITE_DEV_HOST, locationHost: location.host };
  return buildCandidates(env, pairTarget ?? loadConn());
}

function conn(): Connection {
  if (connection) return connection;
  connection = new Connection({
    WebSocketCtor: WebSocket as unknown as WebSocketCtor,
    now: () => Date.now(),
    setTimeout: (fn, ms) => window.setTimeout(fn, ms),
    clearTimeout: (h) => window.clearTimeout(h as number),
    candidates,
    token: () => loadConn()?.token ?? null,
    native: isNative(),
    clientInfo: clientInfo(),
    onWelcome: applyWelcome,
    onPatch: applyPatch,
    onPressResult: () => {},
    onStatus: (c, via) => {
      if (pairTarget) return; // the pairing screen stays up until pairing finishes
      setState({ conn: c, via });
    },
    onFatal: (code: ErrorCode) => {
      if (code === "PROTOCOL_VERSION") setState({ conn: "update_required" });
      else unpair(); // UNAUTHORIZED / DEVICE_REVOKED: the saved pairing is no longer valid
    },
  });
  return connection;
}

export function pairErrorText(code: ConnFailure["code"]): string {
  switch (code) {
    case "PAIR_CODE_INVALID":
      return "Code is wrong or expired. Generate a new one on the PC.";
    case "RATE_LIMITED":
      return "Too many attempts. Wait a minute.";
    default:
      return "Can't reach the PC. Check Wi-Fi or the USB cable.";
  }
}

/** Pairs with the PC; on success the connection continues with the new token. Rejects with a ConnFailure. */
export async function pairWith(hosts: string[], port: number, code: string, agentName: string): Promise<void> {
  const c = conn();
  pairTarget = { hosts, port };
  try {
    const paired = await c.pair(code);
    saveConn({ token: paired.token, deviceId: paired.deviceId, hosts, port, agentName });
  } catch (e) {
    pairTarget = null;
    if (loadConn()) c.start();
    else setState({ conn: "unpaired" });
    throw e;
  }
  pairTarget = null;
  setState({ conn: "connecting", lastError: null });
}

export function unpair(): void {
  clearConn();
  conn().stop();
  setState({ conn: "unpaired", via: null, agentName: null, state: null, rev: 0, pageIndex: 0, lastError: null });
}

export function press(actionId: string): Promise<void> {
  return conn().press(actionId);
}

export function dismiss(notifId: string): void {
  conn().dismiss(notifId);
}

export function setPage(pageIndex: number): void {
  setState({ pageIndex });
}

export function init(): void {
  document.addEventListener("visibilitychange", () => {
    if (document.visibilityState === "visible") conn().wake();
  });

  if (location.hash.includes("c=")) {
    const parsed = parsePairUrl(location.href);
    if (parsed) {
      history.replaceState(null, "", location.pathname + location.search); // erase the code from the address bar
      pairWith(parsed.hosts, parsed.port, parsed.code, parsed.agentName).catch((e: ConnFailure) => {
        setState({ conn: "unpaired", lastError: pairErrorText(e.code) });
      });
      return;
    }
  }

  if (loadConn()) {
    setState({ conn: "connecting" });
    conn().start();
  } else {
    setState({ conn: "unpaired" });
  }
}
