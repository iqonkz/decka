import { useEffect, useState } from "react";
import { isEmptySpot, MENU_HOLD_MS, MENU_HOLD_SLOP_PX } from "./menuModel";

type Point = { x: number; y: number };

/** The part of `window` the gesture needs; a fake one is passed in tests. */
export interface GestureHost {
  addEventListener(type: string, fn: (e: PointerEvent) => void, capture: boolean): void;
  removeEventListener(type: string, fn: (e: PointerEvent) => void, capture: boolean): void;
  setTimeout(fn: () => void, ms: number): number;
  clearTimeout(id: number): void;
}

/**
 * The hold logic: a pointerdown on empty space starts a timer, a move beyond the slop or a lift cancels it.
 * A pointerdown that comes from a tile (`closest('[data-tile]')`, see `isEmptySpot`) never starts it.
 * Returns the function that detaches the listeners.
 */
export function attachMenuGesture(host: GestureHost, onOpen: () => void, setHold: (p: Point | null) => void): () => void {
  let timer: number | null = null;
  let origin: Point | null = null;

  const stop = () => {
    if (timer !== null) host.clearTimeout(timer);
    timer = null;
    origin = null;
    setHold(null);
  };
  const onDown = (e: PointerEvent) => {
    stop();
    if (!isEmptySpot(e.target)) return;
    origin = { x: e.clientX, y: e.clientY };
    setHold(origin);
    timer = host.setTimeout(() => {
      stop();
      onOpen();
    }, MENU_HOLD_MS);
  };
  const onMove = (e: PointerEvent) => {
    if (origin && Math.hypot(e.clientX - origin.x, e.clientY - origin.y) > MENU_HOLD_SLOP_PX) stop();
  };
  host.addEventListener("pointerdown", onDown, true);
  host.addEventListener("pointermove", onMove, true);
  host.addEventListener("pointerup", stop, true);
  host.addEventListener("pointercancel", stop, true);
  return () => {
    stop();
    host.removeEventListener("pointerdown", onDown, true);
    host.removeEventListener("pointermove", onMove, true);
    host.removeEventListener("pointerup", stop, true);
    host.removeEventListener("pointercancel", stop, true);
  };
}

/**
 * A long press on empty space opens the phone menu. Returns the point where the finger is while the hold runs (for
 * the ring that shows its progress), or `null`. Counted on `window` in the capture phase, so single taps and
 * swipes are untouched.
 */
export function useMenuGesture(onOpen: () => void, enabled: boolean): Point | null {
  const [hold, setHold] = useState<Point | null>(null);

  useEffect(() => {
    if (!enabled) return;
    return attachMenuGesture(
      {
        addEventListener: (type, fn, capture) => window.addEventListener(type, fn as EventListener, capture),
        removeEventListener: (type, fn, capture) => window.removeEventListener(type, fn as EventListener, capture),
        setTimeout: (fn, ms) => window.setTimeout(fn, ms),
        clearTimeout: (id) => window.clearTimeout(id),
      },
      onOpen,
      setHold,
    );
  }, [onOpen, enabled]);

  return hold;
}
