import { useSyncExternalStore } from "react";
import type { AiInfo, FullState, NotifLevel, Notification, PatchMsg, PermRequest, SectionName, WelcomeMsg } from "@decka/shared";
import { popBack, pushBack } from "../deck/backStack";
import { decideContextPage } from "../deck/context";

export type ConnState = "unpaired" | "connecting" | "online" | "offline" | "update_required";

export interface Toast {
  id: number;
  level: NotifLevel;
  title: string;
  body?: string;
  until: number;
  /** A notification of Windows that a tap can open (DATA_MODEL.md 10.4). */
  notifId?: string;
}

export interface AppState {
  conn: ConnState;
  via: "usb" | "lan" | null;
  agentName: string | null;
  /** `welcome.agent.version`: the version of the PC agent (the phone menu compares it with its own). */
  agentVersion: string | null;
  rev: number;
  state: FullState | null;
  pageIndex: number;
  /** The page the user picked by hand (swipe, `navigate` button); context pages return here. */
  homePageIndex: number;
  /** Id of the page opened automatically for the active PC application, or null. */
  autoPageId: string | null;
  /** Ids of the pages the user left by hand, newest last, at most 10: what a `$back` key walks through (DATA_MODEL.md 3.8). */
  backStack: string[];
  /** Time of the last touch (ms); a recent touch postpones automatic page switches. */
  lastTouch: number;
  /** Name of the page just opened automatically, shown for a moment. */
  hint: { id: number; text: string } | null;
  /** The open popup group: its page and the box of the key that opened it (DESIGN.md 4.1). */
  popup: { pageId: string; anchor: { x: number; y: number; width: number; height: number } } | null;
  toasts: Toast[];
  lastError: string | null;
  /** Ids of the permission questions that were answered here or put away; they stay out of the sheet until they leave `ai.pending`. */
  permClosed: string[];
  /** The list of answered questions is open (a tap on the `ai` widget). */
  permHistoryOpen: boolean;
}

const SECTIONS: readonly string[] = ["layout", "sys", "statuses", "notifications", "vars", "focus", "media", "icons", "weather", "obs", "ai", "twitch", "youtube", "strip", "battery", "mail"];
const MAX_TOASTS = 3;
const DEFAULT_TOAST_MS = 8000;

const initial = (): AppState => ({
  conn: "connecting",
  via: null,
  agentName: null,
  agentVersion: null,
  rev: 0,
  state: null,
  pageIndex: 0,
  homePageIndex: 0,
  autoPageId: null,
  backStack: [],
  lastTouch: 0,
  hint: null,
  popup: null,
  toasts: [],
  lastError: null,
  permClosed: [],
  permHistoryOpen: false,
});

let current: AppState = initial();
const listeners = new Set<() => void>();
let toastCounter = 0;
const attentionListeners = new Set<(level: NotifLevel) => void>();

/** Called for every new notification toast (the screen saver / dimming wakes up on it). */
export function onAttention(fn: (level: NotifLevel) => void): () => void {
  attentionListeners.add(fn);
  return () => attentionListeners.delete(fn);
}

let toastGate: ((level: NotifLevel) => boolean) | null = null;

/** Decides per new notification whether it may show a toast (the screen saver keeps quiet for info/success). */
export function setToastGate(gate: ((level: NotifLevel) => boolean) | null): void {
  toastGate = gate;
}

export function getState(): AppState {
  return current;
}

export function setState(update: Partial<AppState> | ((s: AppState) => Partial<AppState>)): void {
  const partial = typeof update === "function" ? update(current) : update;
  current = { ...current, ...partial };
  listeners.forEach((fn) => fn());
}

export function subscribe(fn: () => void): () => void {
  listeners.add(fn);
  return () => listeners.delete(fn);
}

export function useAppState<T>(selector: (s: AppState) => T): T {
  const snapshot = () => selector(current);
  return useSyncExternalStore(subscribe, snapshot, snapshot); // the server snapshot lets components render to a string in tests
}

/** Test helper. */
export function resetState(): void {
  current = initial();
  toastCounter = 0;
  listeners.forEach((fn) => fn());
}

/** A page index that no longer exists falls back to the first page. */
function fixPageIndex(pageIndex: number, state: FullState): number {
  return pageIndex >= state.layout.pages.length ? 0 : pageIndex;
}

export function applyWelcome(msg: WelcomeMsg): void {
  setState((s) => ({
    state: msg.state,
    rev: msg.rev,
    conn: "online",
    agentName: msg.agent.name,
    agentVersion: msg.agent.version,
    pageIndex: fixPageIndex(s.pageIndex, msg.state),
    homePageIndex: fixPageIndex(s.homePageIndex, msg.state),
    lastError: null,
    permClosed: [], // a new connection starts the sheet afresh: what is still pending is shown again
  }));
  runContext();
}

export function applyPatch(msg: PatchMsg): void {
  const s = current;
  if (!s.state || msg.rev <= s.rev) return;
  if (!SECTIONS.includes(msg.section)) return; // unknown sections are ignored (forward compatibility)
  const next = { ...s.state, [msg.section as SectionName]: msg.value } as FullState;
  const layoutChanged = msg.section === "layout";
  setState({
    state: next,
    rev: msg.rev,
    pageIndex: layoutChanged ? fixPageIndex(s.pageIndex, next) : s.pageIndex,
    homePageIndex: layoutChanged ? fixPageIndex(s.homePageIndex, next) : s.homePageIndex,
    // An open group whose page is gone (or is no longer a popup page) closes.
    ...(layoutChanged && s.popup && !next.layout.pages.some((p) => p.id === s.popup!.pageId && p.popup === true) ? { popup: null } : {}),
  });
  if (msg.section === "notifications") toastNewNotifications(s.state.notifications, msg.value as Notification[]);
  if (msg.section === "ai") onAiPatch(s.state.ai?.pending ?? [], (msg.value as AiInfo | null)?.pending ?? []);
  if (msg.section === "focus" || layoutChanged) runContext();
}

/** A question that left `pending` (answered on the PC, timed out) is forgotten here, so the list does not grow. */
function onAiPatch(before: PermRequest[], after: PermRequest[]): void {
  const ids = new Set(after.map((p) => p.reqId));
  if (current.permClosed.some((id) => !ids.has(id))) setState({ permClosed: current.permClosed.filter((id) => ids.has(id)) });
  // A new question wakes the screen from dimming, like a warning (DATA_MODEL.md 6.8).
  const known = new Set(before.map((p) => p.reqId));
  if (after.some((p) => !known.has(p.reqId))) signalAttention("warning");
}

/** The questions the sheet still shows: those that were not answered or put away, oldest first. */
export function openPerms(pending: readonly PermRequest[] | undefined, closed: readonly string[]): PermRequest[] {
  return (pending ?? []).filter((p) => !closed.includes(p.reqId));
}

/** The question on the sheet (the oldest open one) and how many are open; `null` = no sheet. */
export function currentPerm(s: AppState): { request: PermRequest; total: number } | null {
  const open = openPerms(s.state?.ai?.pending, s.permClosed);
  const request = open[0];
  return request ? { request, total: open.length } : null;
}

/** The sheet closes for this question (it was answered, or put away with a tap beside it); the badge brings it back. */
export function markPermAnswered(reqId: string): void {
  if (!current.permClosed.includes(reqId)) setState({ permClosed: [...current.permClosed, reqId] });
}

/** The badge of the `ai` widget: the questions that were put away come back. */
export function openPermHistory(): void {
  if (!current.permHistoryOpen) setState({ permHistoryOpen: true });
}

export function closePermHistory(): void {
  if (current.permHistoryOpen) setState({ permHistoryOpen: false });
}

export function reopenPerm(): void {
  if (current.permClosed.length > 0) setState({ permClosed: [] });
}

let hintCounter = 0;

/** DATA_MODEL.md 6.1: applies `decideContextPage` to the current state. Does not touch the idle timer. */
export function runContext(now: number = Date.now()): void {
  const s = current;
  if (!s.state) return;
  const d = decideContextPage({
    pages: s.state.layout.pages,
    display: s.state.layout.display,
    focusApp: s.state.focus,
    pageIndex: s.pageIndex,
    homePageIndex: s.homePageIndex,
    autoPageId: s.autoPageId,
    now,
    lastTouch: s.lastTouch,
  });
  if (d.pageIndex === s.pageIndex && d.autoPageId === s.autoPageId && d.hint === null) return;
  setState({
    pageIndex: d.pageIndex,
    autoPageId: d.autoPageId,
    ...(d.hint !== null ? { hint: { id: ++hintCounter, text: d.hint } } : {}),
    ...(d.pageIndex !== s.pageIndex ? { popup: null } : {}), // an automatic page change closes an open group
  });
}

/** The user chose a page (swipe, `navigate` button): it becomes home and the automatic page is forgotten. */
export function navigateManually(pageIndex: number): void {
  const s = current;
  const from = s.state?.layout.pages[s.pageIndex]?.id;
  // The page left by hand goes on the back stack; an automatic (context) change never does, and staying put writes nothing.
  const backStack = from !== undefined && pageIndex !== s.pageIndex ? pushBack(s.backStack, from) : s.backStack;
  setState({ pageIndex, homePageIndex: pageIndex, autoPageId: null, hint: null, popup: null, backStack });
}

/** The `$back` key: the page the user came from; with an empty stack (or only pages that are gone) the first page. */
export function goBack(): void {
  const s = current;
  const pages = s.state?.layout.pages ?? [];
  const { pageId, stack } = popBack(s.backStack, (id) => pages.some((p) => p.id === id && p.popup !== true));
  const index = pageId === null ? 0 : pages.findIndex((p) => p.id === pageId);
  setState({ pageIndex: index, homePageIndex: index, autoPageId: null, hint: null, popup: null, backStack: stack });
}

/** Something wants the phone awake (the timer ran down): same as an attention notification, without a notification. */
export function signalAttention(level: NotifLevel): void {
  attentionListeners.forEach((fn) => fn(level));
}

export function openPopup(pageId: string, anchor: { x: number; y: number; width: number; height: number }): void {
  setState({ popup: { pageId, anchor } });
}

export function closePopup(): void {
  if (current.popup !== null) setState({ popup: null });
}

export function noteTouch(now: number = Date.now()): void {
  setState({ lastTouch: now });
}

export function clearHint(id: number): void {
  if (current.hint?.id === id) setState({ hint: null });
}

/** One 8 s toast per id that was not in the previous value (oldest first). */
function toastNewNotifications(before: Notification[], after: Notification[]): void {
  const known = new Set(before.map((n) => n.id));
  for (const n of [...after].reverse()) {
    if (known.has(n.id)) continue;
    if (toastGate && !toastGate(n.level)) continue;
    const windows = n.source === "windows";
    // A notification of Windows is shown for 4 s, and only when the page has no notifications widget to show it (10.4).
    if (windows && pageHasNotificationsWidget()) continue;
    const toast: NewToast = { level: n.level, title: n.app && windows ? `${n.app}: ${n.title}` : n.title };
    if (n.body !== undefined) toast.body = n.body;
    if (windows) toast.ms = WINDOWS_TOAST_MS;
    if (windows && n.openable === true) toast.notifId = n.id;
    pushToast(toast);
    attentionListeners.forEach((fn) => fn(n.level));
  }
}

const WINDOWS_TOAST_MS = 4000;

/** Does the page in front have a notifications widget? */
function pageHasNotificationsWidget(): boolean {
  const page = current.state?.layout.pages[current.pageIndex];
  return page?.items.some((i) => i.kind === "notifications") === true;
}

export interface NewToast {
  level: NotifLevel;
  title: string;
  body?: string;
  notifId?: string;
  /** Lifetime in ms (default 8000). */
  ms?: number;
}

export function pushToast(t: NewToast, now: number = Date.now()): void {
  const toast: Toast = { id: ++toastCounter, level: t.level, title: t.title, until: now + (t.ms ?? DEFAULT_TOAST_MS) };
  if (t.body !== undefined) toast.body = t.body;
  if (t.notifId !== undefined) toast.notifId = t.notifId;
  setState((s) => ({ toasts: [...s.toasts, toast].slice(-MAX_TOASTS) }));
}

export function dismissToast(id: number): void {
  setState((s) => ({ toasts: s.toasts.filter((t) => t.id !== id) }));
}

export function expireToasts(now: number): void {
  if (!current.toasts.some((t) => t.until <= now)) return;
  setState((s) => ({ toasts: s.toasts.filter((t) => t.until > now) }));
}
