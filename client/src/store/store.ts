import { useSyncExternalStore } from "react";
import type { FullState, NotifLevel, Notification, PatchMsg, SectionName, WelcomeMsg } from "@decka/shared";
import { decideContextPage } from "../deck/context";

export type ConnState = "unpaired" | "connecting" | "online" | "offline" | "update_required";

export interface Toast {
  id: number;
  level: NotifLevel;
  title: string;
  body?: string;
  until: number;
}

export interface AppState {
  conn: ConnState;
  via: "usb" | "lan" | null;
  agentName: string | null;
  rev: number;
  state: FullState | null;
  pageIndex: number;
  /** The page the user picked by hand (swipe, `navigate` button); context pages return here. */
  homePageIndex: number;
  /** Id of the page opened automatically for the active PC application, or null. */
  autoPageId: string | null;
  /** Time of the last touch (ms); a recent touch postpones automatic page switches. */
  lastTouch: number;
  /** Name of the page just opened automatically, shown for a moment. */
  hint: { id: number; text: string } | null;
  toasts: Toast[];
  lastError: string | null;
}

const SECTIONS: readonly string[] = ["layout", "sys", "statuses", "notifications", "vars", "focus", "media", "icons"];
const MAX_TOASTS = 3;
const DEFAULT_TOAST_MS = 8000;

const initial = (): AppState => ({
  conn: "connecting",
  via: null,
  agentName: null,
  rev: 0,
  state: null,
  pageIndex: 0,
  homePageIndex: 0,
  autoPageId: null,
  lastTouch: 0,
  hint: null,
  toasts: [],
  lastError: null,
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
    pageIndex: fixPageIndex(s.pageIndex, msg.state),
    homePageIndex: fixPageIndex(s.homePageIndex, msg.state),
    lastError: null,
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
  });
  if (msg.section === "notifications") toastNewNotifications(s.state.notifications, msg.value as Notification[]);
  if (msg.section === "focus" || layoutChanged) runContext();
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
  });
}

/** The user chose a page (swipe, `navigate` button): it becomes home and the automatic page is forgotten. */
export function navigateManually(pageIndex: number): void {
  setState({ pageIndex, homePageIndex: pageIndex, autoPageId: null, hint: null });
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
    const toast: NewToast = { level: n.level, title: n.title };
    if (n.body !== undefined) toast.body = n.body;
    pushToast(toast);
    attentionListeners.forEach((fn) => fn(n.level));
  }
}

export interface NewToast {
  level: NotifLevel;
  title: string;
  body?: string;
  /** Lifetime in ms (default 8000). */
  ms?: number;
}

export function pushToast(t: NewToast, now: number = Date.now()): void {
  const toast: Toast = { id: ++toastCounter, level: t.level, title: t.title, until: now + (t.ms ?? DEFAULT_TOAST_MS) };
  if (t.body !== undefined) toast.body = t.body;
  setState((s) => ({ toasts: [...s.toasts, toast].slice(-MAX_TOASTS) }));
}

export function dismissToast(id: number): void {
  setState((s) => ({ toasts: s.toasts.filter((t) => t.id !== id) }));
}

export function expireToasts(now: number): void {
  if (!current.toasts.some((t) => t.until <= now)) return;
  setState((s) => ({ toasts: s.toasts.filter((t) => t.until > now) }));
}
