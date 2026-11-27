// The single place where Connection, the store and local storage meet (ARCHITECTURE.md 4.4).
import { parsePairUrl, t } from "@decka/shared";
import { buildCandidates } from "../net/candidates";
import { Connection, type ConnFailure, type FatalCode, type WebSocketCtor } from "../net/connection";
import { hapticEvent } from "../platform/haptics";
import { installApk } from "../platform/install";
import { clientInfo, isNative } from "../platform/native";
import { applyPatch, applyWelcome, getState, markPermAnswered, navigateManually, setState } from "../store/store";
import { ApkAssembler, ApkError } from "../update/apkAssembler";
import { clearConn, loadConn, renameCurrent, saveConn, selectPc } from "../store/storage";
import { installVoiceGoto } from "../voice/goto";
import { installPageShown } from "./pageShown";
import { startPhoneBatteryMonitor } from "./phoneBattery";

/** Where the next pairing attempt goes; null when not pairing. */
let pairTarget: { hosts: string[]; port: number } | null = null;
let connection: Connection | null = null;
/** The first-connection screen was opened from "Connect again" (a saved PC exists). */
let repairing = false;

function candidates(): string[] {
  const env = { native: isNative(), devHost: import.meta.env.VITE_DEV_HOST, locationHost: location.host };
  return buildCandidates(env, pairTarget ?? loadConn());
}

function conn(): Connection {
  if (connection) return connection;
  // A connection that was replaced (another PC was chosen) must not change what is shown any more.
  const mine: Connection = new Connection({
    WebSocketCtor: WebSocket as unknown as WebSocketCtor,
    now: () => Date.now(),
    setTimeout: (fn, ms) => window.setTimeout(fn, ms),
    clearTimeout: (h) => window.clearTimeout(h as number),
    candidates,
    token: () => loadConn()?.token ?? null,
    deviceId: () => loadConn()?.deviceId ?? null,
    native: isNative(),
    clientInfo: clientInfo(),
    onWelcome: (welcome) => {
      if (connection !== mine) return;
      applyWelcome(welcome);
      renameCurrent(welcome.agent.name);
    },
    onPatch: (patch) => {
      if (connection === mine) applyPatch(patch);
    },
    onPressResult: () => {},
    onStatus: (c, via) => {
      if (pairTarget || connection !== mine) return; // the pairing screen stays up until pairing finishes
      setState({ conn: c, via });
    },
    onFatal: (code: FatalCode) => {
      if (connection !== mine) return;
      if (code === "SERVER_MISMATCH") setState({ conn: pairTarget ? "unpaired" : "not_your_pc" });
      else if (code === "PROTOCOL_VERSION") setState({ conn: "update_required" });
      // UNAUTHORIZED / DEVICE_REVOKED: the PC does not know this phone. No background attempts; the card offers the way out and
      // the saved PC stays until a new connection has succeeded (DESIGN.md 4.13).
      else setState({ conn: "rejected" });
    },
  });
  connection = mine;
  return mine;
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
    // The saved PC is kept. From "Connect again" the screen stays up (the user decides); at start-up the old link goes on.
    if (repairing) setState({ conn: "repair" });
    else if (loadConn()) c.start();
    else setState({ conn: "unpaired" });
    throw e;
  }
  pairTarget = null;
  repairing = false;
  // The keys of the PC that was shown belong to it: nothing of it stays while the new one connects.
  leave();
  setState({ ...NOTHING, conn: "connecting" });
  conn().start(); // the pairing socket is closed; continue as a device with the saved token
}

/** The "Retry" of the screen "This is not your PC" and "Try again" of the card: one new attempt with the saved pairing. */
export function retryConnection(): void {
  repairing = false;
  setState({ conn: "connecting" });
  conn().start();
}

/** "Connect again": the first-connection screen (QR, code). The saved PC is not touched until the new pairing has succeeded. */
export function connectAgain(): void {
  repairing = true;
  conn().stop();
  setState({ conn: "repair", lastError: null });
}

/** Back from "Connect again" to the saved PC. */
export function cancelConnectAgain(): void {
  if (loadConn()) retryConnection();
  else unpair();
}

/** Nothing of the PC that was shown stays on the screen. */
const NOTHING = { via: null, agentName: null, agentVersion: null, state: null, rev: 0, pageIndex: 0, homePageIndex: 0, autoPageId: null, hint: null, popup: null, lastError: null };

/** Drops the connection; the next `conn()` is a new one, for the PC that is current then. */
function leave(): void {
  connection?.stop();
  connection = null;
}

/** T32.4: another PC of the list, without pairing again. */
export function switchPc(deviceId: string): void {
  if (!selectPc(deviceId)) return;
  repairing = false;
  pairTarget = null;
  leave();
  setState({ ...NOTHING, conn: "connecting" });
  conn().start();
}

/** Forgets the PC the phone is on; with other PCs in the list the first of them takes its place. */
export function unpair(): void {
  repairing = false;
  clearConn();
  leave();
  const next = loadConn();
  setState({ ...NOTHING, conn: next ? "connecting" : "unpaired" });
  if (next) conn().start();
}

export function press(actionId: string): Promise<void> {
  return conn().press(actionId);
}

export function dismiss(notifId: string): void {
  conn().dismiss(notifId);
}

/** A tap on a notification of Windows: the PC opens its program (DATA_MODEL.md 10.4). */
export function toggleListItem(listId: string, itemId: string): void {
  conn().toggleListItem(listId, itemId);
}

/** M24 (DATA_MODEL.md 14): adjusts per-track synced lyrics offset on PC (`lyrics_offset`). */
export function sendLyricsOffset(trackKey: string, deltaMs: -500 | 500): void {
  conn().lyricsOffset(trackKey, deltaMs);
}

export function openNotification(notifId: string): void {
  conn().openNotification(notifId);
}

/** M21: a `voice_*` message to the PC; `false` when there is no live channel. */
export function sendVoice(msg: Parameters<Connection["sendVoice"]>[0]): boolean {
  return conn().sendVoice(msg);
}

/** Answers a permission question; the sheet closes at once, without waiting for the new `ai` section. */
export function answerPerm(reqId: string, decision: "allow" | "allow_always" | "deny" | "pc" | "reply", reply?: number): void {
  markPermAnswered(reqId);
  conn().permAnswer(reqId, decision, reply);
}

/**
 * T20.4: fetches the new app from the PC piece by piece (inside the encrypted channel), checks its SHA-256 here and hands it to
 * Android's installer. Only the announced file counts: any piece that does not fit the announcement ends the download.
 */
export async function startAppUpdate(): Promise<void> {
  const info = getState().state?.update?.app;
  const phase = getState().appUpdate.phase;
  if (!info || phase === "downloading" || phase === "verifying") return;
  setState({ appUpdate: { phase: "downloading", pct: 0 } });
  try {
    const assembler = new ApkAssembler(info.size, info.sha256);
    for (let offset = assembler.next; offset !== null; offset = assembler.next) {
      const chunk = await conn().apkGet(offset);
      if (chunk.total !== info.size || chunk.sha256 !== info.sha256) throw new ApkError("BAD_CHUNK");
      assembler.add(chunk.offset, Uint8Array.from(atob(chunk.data), (c) => c.charCodeAt(0)));
      const pct = assembler.percent;
      const shown = getState().appUpdate;
      if (shown.phase === "downloading" && shown.pct !== pct) setState({ appUpdate: { phase: "downloading", pct } });
    }
    setState({ appUpdate: { phase: "verifying" } });
    const bytes = assembler.finish();
    await installApk(bytes, info.sha256);
    setState({ appUpdate: { phase: "idle" } });
  } catch (e) {
    setState({ appUpdate: { phase: "error", kind: e instanceof ApkError && e.code === "HASH_MISMATCH" ? "bad" : "failed" } });
  }
}

/** Manual navigation (swipe, `navigate` button). */
export function setPage(pageIndex: number, showNameHint = false): void {
  hapticEvent({ type: "page_change", manual: true });
  navigateManually(pageIndex, showNameHint);
}

export function init(): void {
  installVoiceGoto();
  installPageShown((pageId) => conn().pageShown(pageId));
  startPhoneBatteryMonitor((battery, charging) => conn().sendPhoneStatus(battery, charging));
  document.addEventListener("visibilitychange", () => {
    if (document.visibilityState === "visible") conn().wake();
  });
  window.addEventListener("online", () => conn().wake());

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
