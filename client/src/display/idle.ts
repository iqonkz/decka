import { useCallback, useEffect, useRef, useState } from "react";
import type { DisplaySettings } from "@decka/shared";
import { onAttention, setToastGate, useAppState } from "../store/store";

export type DisplayMode = "active" | "dim" | "sleep";

/** Touches and attention events younger than this always keep the screen fully on. */
const ACTIVE_GRACE_MS = 10_000;
/** A lost connection this old puts the screen to sleep, whatever the timers say. */
const OFFLINE_SLEEP_MS = 30_000;
const TICK_MS = 1000;

export interface ModeInput {
  now: number;
  lastActivity: number;
  display: DisplaySettings;
  offlineSince: number | null;
  /** Minutes since local midnight. */
  nowMinutes: number;
}

function toMinutes(hhmm: string): number {
  const [h = "0", m = "0"] = hhmm.split(":");
  return Number(h) * 60 + Number(m);
}

/** True when `nowMinutes` is inside the night interval; the interval may cross midnight, `from === to` is empty. */
export function inNight(night: DisplaySettings["night"], nowMinutes: number): boolean {
  const from = toMinutes(night.from);
  const to = toMinutes(night.to);
  if (from === to) return false;
  return from < to ? nowMinutes >= from && nowMinutes < to : nowMinutes >= from || nowMinutes < to;
}

/** ARCHITECTURE.md 5.6; the rules apply in this order. */
export function computeMode(input: ModeInput): DisplayMode {
  const { now, lastActivity, display, offlineSince, nowMinutes } = input;
  const idle = now - lastActivity;
  if (idle < ACTIVE_GRACE_MS) return "active";
  if (offlineSince !== null && now - offlineSince >= OFFLINE_SLEEP_MS) return "sleep";
  if (display.night.enabled && inNight(display.night, nowMinutes)) return "sleep";
  if (display.sleepAfterSec > 0 && idle >= display.sleepAfterSec * 1000) return "sleep";
  if (display.dimAfterSec > 0 && idle >= display.dimAfterSec * 1000) return "dim";
  return "active";
}

let currentMode: DisplayMode = "active";
let wokeByLastTouch = false;

/** True when the latest touch found the screen dimmed or asleep (it only woke it up; no haptic feedback). */
export function lastTouchWokeScreen(): boolean {
  return wokeByLastTouch;
}

/** Latest mode computed by `useDisplayMode` (read by the toast gate). */
export function getDisplayMode(): DisplayMode {
  return currentMode;
}

/**
 * Tracks the last touch and recomputes the mode every second and right after a touch or an
 * attention-level notification (`warning`/`error`). `null` display settings mean "always active".
 */
export function useDisplayMode(display: DisplaySettings | null): DisplayMode {
  const conn = useAppState((s) => s.conn);
  const [mode, setMode] = useState<DisplayMode>("active");
  const lastActivity = useRef(Date.now());
  const offlineSince = useRef<number | null>(null);
  const displayRef = useRef(display);
  displayRef.current = display;

  const recompute = useCallback(() => {
    const settings = displayRef.current;
    const now = new Date();
    const next = settings
      ? computeMode({
          now: now.getTime(),
          lastActivity: lastActivity.current,
          display: settings,
          offlineSince: offlineSince.current,
          nowMinutes: now.getHours() * 60 + now.getMinutes(),
        })
      : "active";
    currentMode = next;
    setMode(next);
  }, []);

  useEffect(() => {
    if (conn === "online") offlineSince.current = null;
    else if (conn === "offline" && offlineSince.current === null) offlineSince.current = Date.now();
    recompute();
  }, [conn, recompute]);

  useEffect(() => {
    const touch = () => {
      lastActivity.current = Date.now();
      recompute();
    };
    // Registered first, so it runs before `touch` changes the mode: was the screen dim or asleep for this touch?
    const noteWake = () => {
      wokeByLastTouch = currentMode !== "active";
    };
    window.addEventListener("pointerdown", noteWake, true);
    window.addEventListener("pointerdown", touch, true);
    const stopAttention = onAttention((level) => {
      if (level === "warning" || level === "error") touch();
    });
    // Info and success notifications do not wake a sleeping screen: no toast, the feed keeps them.
    setToastGate((level) => level === "warning" || level === "error" || currentMode !== "sleep");
    const timer = window.setInterval(recompute, TICK_MS);
    return () => {
      window.removeEventListener("pointerdown", noteWake, true);
      window.removeEventListener("pointerdown", touch, true);
      stopAttention();
      setToastGate(null);
      window.clearInterval(timer);
      currentMode = "active";
    };
  }, [recompute]);

  useEffect(() => {
    recompute(); // new settings from the agent apply at once
  }, [display, recompute]);

  return mode;
}
