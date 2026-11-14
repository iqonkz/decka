import { useSyncExternalStore } from "react";
import type { FullState, NotifLevel, Notification, PatchMsg, SectionName, WelcomeMsg } from "@decka/shared";

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
  toasts: Toast[];
  lastError: string | null;
}

const SECTIONS: readonly string[] = ["layout", "sys", "statuses", "notifications", "vars"];
const MAX_TOASTS = 3;
const DEFAULT_TOAST_MS = 8000;

const initial = (): AppState => ({
  conn: "connecting",
  via: null,
  agentName: null,
  rev: 0,
  state: null,
  pageIndex: 0,
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
  return useSyncExternalStore(subscribe, () => selector(current));
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
    lastError: null,
  }));
}

export function applyPatch(msg: PatchMsg): void {
  const s = current;
  if (!s.state || msg.rev <= s.rev) return;
  if (!SECTIONS.includes(msg.section)) return; // unknown sections are ignored (forward compatibility)
  const next = { ...s.state, [msg.section as SectionName]: msg.value } as FullState;
  setState({
    state: next,
    rev: msg.rev,
    pageIndex: msg.section === "layout" ? fixPageIndex(s.pageIndex, next) : s.pageIndex,
  });
  if (msg.section === "notifications") toastNewNotifications(s.state.notifications, msg.value as Notification[]);
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
