import type { OrientationLockType } from "@capacitor/screen-orientation";
import type { Orientation } from "@decka/shared";

/** Which way up the phone stands (DATA_MODEL.md 10.9). Kept on this phone only. */
export type Turn = "auto" | "normal" | "flipped";

export const TURN_KEY = "decka.turn.v1";

function safeStorage(): Storage | undefined {
  try {
    return window.localStorage;
  } catch {
    return undefined;
  }
}

/** The remembered turn; `auto` when there is none or the value is not one (storage may be blocked). */
export function loadTurn(storage: Pick<Storage, "getItem"> | undefined = safeStorage()): Turn {
  try {
    const value = storage?.getItem(TURN_KEY);
    return value === "normal" || value === "flipped" ? value : "auto";
  } catch {
    return "auto";
  }
}

export function saveTurn(turn: Turn, storage: Pick<Storage, "setItem"> | undefined = safeStorage()): void {
  try {
    storage?.setItem(TURN_KEY, turn);
  } catch {
    // blocked storage: the choice lasts until the app is closed
  }
}

/**
 * The lock the screen gets: `auto` lets the sensor pick either side of the layout's orientation, `normal` the primary side,
 * `flipped` the secondary one (DATA_MODEL.md 10.9).
 */
export function orientationLock(orientation: Orientation | undefined, turn: Turn): OrientationLockType {
  const base = orientation === "portrait" ? "portrait" : "landscape";
  if (turn === "auto") return base;
  return turn === "normal" ? `${base}-primary` : `${base}-secondary`;
}
