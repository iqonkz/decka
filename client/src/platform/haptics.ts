// Haptic feedback (DESIGN.md 3a): one module, errors swallowed, a per-phone switch kept in localStorage.
import { Haptics, ImpactStyle, NotificationType } from "@capacitor/haptics";
import { isNative } from "./native";

export type HapticKind = "tap" | "confirm" | "error" | "open" | "page" | "menu";

export type HapticEvent =
  | { type: "touch"; /** the touch only woke the screen from dim or sleep */ wokeScreen: boolean }
  | { type: "confirm" }
  | { type: "press_result"; ok: boolean }
  | { type: "popup_open" }
  | { type: "page_change"; /** by the user (swipe, button), not by a context page */ manual: boolean }
  | { type: "menu" };

export const HAPTICS_KEY = "decka.haptics.v1";

/** Which feedback an event gets, or `null` for none (the rules of DESIGN.md 3a). */
export function hapticFor(event: HapticEvent, ctx: { enabled: boolean }): HapticKind | null {
  if (!ctx.enabled) return null;
  switch (event.type) {
    case "touch":
      return event.wokeScreen ? null : "tap"; // a touch that only wakes the screen does not vibrate
    case "confirm":
      return "confirm";
    case "press_result":
      return event.ok ? null : "error"; // success is silent: the touch already gave feedback
    case "popup_open":
      return "open";
    case "page_change":
      return event.manual ? "page" : null; // a context switch happens without the user
    case "menu":
      return "menu";
  }
}

/** `navigator.vibrate` pattern of a kind, in ms. */
export function vibrationFor(kind: HapticKind): number | number[] {
  switch (kind) {
    case "tap":
    case "open":
      return 10;
    case "confirm":
    case "menu":
      return 20;
    case "error":
      return [30, 60, 30];
    case "page":
      return 5;
  }
}

function safeStorage(): Storage | undefined {
  try {
    return window.localStorage;
  } catch {
    return undefined;
  }
}

/** The phone's switch; on unless it was turned off. */
export function loadHapticsEnabled(storage: Pick<Storage, "getItem"> | undefined = safeStorage()): boolean {
  try {
    return storage?.getItem(HAPTICS_KEY) !== "0";
  } catch {
    return true;
  }
}

export function saveHapticsEnabled(enabled: boolean, storage: Pick<Storage, "setItem"> | undefined = safeStorage()): void {
  try {
    storage?.setItem(HAPTICS_KEY, enabled ? "1" : "0");
  } catch {
    // storage blocked: the switch just is not remembered
  }
}

let enabled = loadHapticsEnabled();

export function isHapticsEnabled(): boolean {
  return enabled;
}

export function setHapticsEnabled(value: boolean): void {
  enabled = value;
  saveHapticsEnabled(value);
}

/** Plays feedback for `kind` in the app (Capacitor) or in a browser that has `vibrate`; never throws. */
export function haptic(kind: HapticKind): void {
  try {
    if (isNative()) {
      void playNative(kind).catch(() => undefined);
      return;
    }
    navigator.vibrate?.(vibrationFor(kind));
  } catch {
    // no vibration available: silently nothing
  }
}

/** The event-level entry: applies the rules and the switch, then plays. */
export function hapticEvent(event: HapticEvent): void {
  const kind = hapticFor(event, { enabled });
  if (kind !== null) haptic(kind);
}

async function playNative(kind: HapticKind): Promise<void> {
  switch (kind) {
    case "tap":
    case "open":
      return Haptics.impact({ style: ImpactStyle.Light });
    case "confirm":
    case "menu":
      return Haptics.impact({ style: ImpactStyle.Medium });
    case "error":
      return Haptics.notification({ type: NotificationType.Error });
    case "page":
      await Haptics.selectionStart();
      await Haptics.selectionChanged();
      return Haptics.selectionEnd();
  }
}
