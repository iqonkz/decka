// The single place where Connection, the store and local storage meet (ARCHITECTURE.md 4.4).
import { parsePairUrl, t } from "@decka/shared";
import { buildCandidates } from "../net/candidates";
import { Connection, type ConnFailure, type FatalCode, type WebSocketCtor } from "../net/connection";
import { hapticEvent } from "../platform/haptics";
import { clientInfo, isNative } from "../platform/native";
import { applyPatch, applyWelcome, markPermAnswered, navigateManually, setState } from "../store/store";
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
    deviceId: () => loadConn()?.deviceId ?? null,
    native: isNative(),
    clientInfo: clientInfo(),
    onWelcome: applyWelcome,
    onPatch: applyPatch,
    onPressResult: () => {},
    onStatus: (c, via) => {
      if (pairTarget) return; // the pairing screen stays up until pairing finishes
      setState({ conn: c, via });
    },
    onFatal: (code: FatalCode) => {
      if (code === "SERVER_MISMATCH") setState({ conn: pairTarget ? "unpaired" : "not_your_pc" });
      else if (code === "PROTOCOL_VERSION") setState({ conn: "update_required" });
      else unpair(); // UNAUTHORIZED / DEVICE_REVOKED: the saved pairing is no longer valid
    },
  });
  return connection;
}

export function pairErrorText(code: ConnFailure["code"]): string {
  switch (code) {
    case "PAIR_CODE_INVALID":
      return t("pair.error.wrong_code");
    case "RATE_LIMITED":
      return t("pair.error.rate");
    case "SERVER_MISMATCH":
      return t("pair.error.mismatch");
    default:
      return t("pair.error.unreachable");
  }
}

/** Pairs with the PC; on success the connection continues with the new token. Rejects with a ConnFailure. */
export async function pairWith(hosts: string[], port: number, code: string, agentName: string, secret?: string): Promise<void> {
  const c = conn();
  pairTarget = { hosts, port };
  try {
    // With the secret of the QR (`s=`) the pairing runs as `pair_qr`, otherwise as `pair_code` (PROTOCOL.md 10).
    const paired = await c.pair(code, secret);
    saveConn({ token: paired.token, deviceId: paired.deviceId, hosts, port, agentName });
  } catch (e) {
    pairTarget = null;
    if (loadConn()) c.start();
    else setState({ conn: "unpaired" });
    throw e;
  }
  pairTarget = null;
  setState({ conn: "connecting", lastError: null });
  c.start(); // the pairing socket is closed; continue as a device with the saved token
}

/** The "Retry" of the screen "This is not your PC": one new attempt with the saved pairing. */
export function retryConnection(): void {
  setState({ conn: "connecting" });
  conn().start();
}

export function unpair(): void {
  clearConn();
  conn().stop();
  setState({ conn: "unpaired", via: null, agentName: null, agentVersion: null, state: null, rev: 0, pageIndex: 0, homePageIndex: 0, autoPageId: null, hint: null, popup: null, lastError: null });
}

export function press(actionId: string): Promise<void> {
  return conn().press(actionId);
}

export function dismiss(notifId: string): void {
  conn().dismiss(notifId);
}

/** A tap on a notification of Windows: the PC opens its program (DATA_MODEL.md 10.4). */
export function openNotification(notifId: string): void {
  conn().openNotification(notifId);
}

/** Answers a permission question; the sheet closes at once, without waiting for the new `ai` section. */
export function answerPerm(reqId: string, decision: "allow" | "allow_always" | "deny" | "pc" | "reply", reply?: number): void {
  markPermAnswered(reqId);
  conn().permAnswer(reqId, decision, reply);
}

/** Manual navigation (swipe, `navigate` button). */
export function setPage(pageIndex: number): void {
  hapticEvent({ type: "page_change", manual: true });
  navigateManually(pageIndex);
}

export function init(): void {
  document.addEventListener("visibilitychange", () => {
    if (document.visibilityState === "visible") conn().wake();
  });

  if (location.hash.includes("c=")) {
    const parsed = parsePairUrl(location.href);
    if (parsed) {
      history.replaceState(null, "", location.pathname + location.search); // erase the code from the address bar
      pairWith(parsed.hosts, parsed.port, parsed.code, parsed.agentName, parsed.secret).catch((e: ConnFailure) => {
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
