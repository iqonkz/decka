import { ScreenOrientation } from "@capacitor/screen-orientation";
import type { Orientation } from "@decka/shared";
import { isNative } from "./native";
import { loadTurn, orientationLock, saveTurn, type Turn } from "./turn";

/** The orientation of the last layout, so that the next start opens in it at once (before any layout arrives). */
export const ORIENTATION_KEY = "decka.orientation.v1";

function safeStorage(): Storage | undefined {
  try {
    return window.localStorage;
  } catch {
    return undefined;
  }
}

/** The remembered orientation; `landscape` when there is none or the value is not one (storage may be blocked). */
export function savedOrientation(storage: Pick<Storage, "getItem"> | undefined = safeStorage()): Orientation {
  try {
    return storage?.getItem(ORIENTATION_KEY) === "portrait" ? "portrait" : "landscape";
  } catch {
    return "landscape";
  }
}

export function saveOrientation(orientation: Orientation, storage: Pick<Storage, "setItem"> | undefined = safeStorage()): void {
  try {
    storage?.setItem(ORIENTATION_KEY, orientation);
  } catch {
    // blocked storage: the next start just opens in landscape until the layout arrives
  }
}

/** Locks the screen in the app (DATA_MODEL.md 3.7, 10.9: the side comes from the turn). In a browser nothing is locked. */
export async function lockOrientation(orientation: Orientation, turn: Turn = loadTurn()): Promise<void> {
  if (!isNative()) return;
  try {
    await ScreenOrientation.lock({ orientation: orientationLock(orientation, turn) });
  } catch {
    // an old WebView or a device that refuses: the screen keeps what it has
  }
}

/** Remembers and locks `orientation` (the layout's `display.orientation`; absent = landscape). */
export function applyOrientation(orientation: Orientation | undefined): void {
  const o = orientation ?? "landscape";
  saveOrientation(o);
  void lockOrientation(o);
}

/** The menu item `Turn`: remembers the choice and applies it at once, without a restart. */
export function applyTurn(turn: Turn): void {
  saveTurn(turn);
  void lockOrientation(savedOrientation(), turn);
}
