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
export type TurnRoute = { via: "sensor"; portrait: boolean } | { via: "plugin"; lock: OrientationLockType };

/** The side of the layout's orientation: `primary` = LANDSCAPE / PORTRAIT, `secondary` = the reverse one. */
export type TurnSide = "primary" | "secondary";

/**
 * Sector table of the own sensor listener (DATA_MODEL.md 10.9, T20.1); `DeckaTurnPlugin.java` repeats it.
 * `angle` is the OrientationEventListener angle 0..359 (-1 = unknown). Outside the sectors the side stays (hysteresis).
 */
export function turnSide(angle: number, current: TurnSide, portrait: boolean): TurnSide {
  if (!Number.isFinite(angle) || angle < 0 || angle > 359) return current;
  if (portrait) {
    if (angle >= 330 || angle <= 30) return "primary";
    if (angle >= 150 && angle <= 210) return "secondary";
  } else {
    if (angle >= 240 && angle <= 300) return "primary";
    if (angle >= 60 && angle <= 120) return "secondary";
  }
  return current;
}

/** `auto` goes to the own `DeckaTurn` plugin (angle listener); `normal` / `flipped` to the screen-orientation plugin (DATA_MODEL.md 10.9). */
export function turnRoute(orientation: Orientation | undefined, turn: Turn): TurnRoute {
  if (turn === "auto") return { via: "sensor", portrait: orientation === "portrait" };
  return { via: "plugin", lock: orientationLock(orientation, turn) };
}

export function orientationLock(orientation: Orientation | undefined, turn: Turn): OrientationLockType {
  const base = orientation === "portrait" ? "portrait" : "landscape";
  if (turn === "auto") return base;
  return turn === "normal" ? `${base}-primary` : `${base}-secondary`;
}
