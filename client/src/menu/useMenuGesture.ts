import { useEffect, useState } from "react";
import { isEmptySpot, MENU_HOLD_MS, MENU_HOLD_SLOP_PX } from "./menuModel";

/**
 * A long press on empty space opens the phone menu. Returns the point where the finger is while the hold runs (for
 * the ring that shows its progress), or `null`. Counted on `window` in the capture phase, so single taps and
 * swipes are untouched.
 */
export function useMenuGesture(onOpen: () => void, enabled: boolean): { x: number; y: number } | null {
  const [hold, setHold] = useState<{ x: number; y: number } | null>(null);

  useEffect(() => {
    if (!enabled) return;
    let timer: number | null = null;
    let origin: { x: number; y: number } | null = null;

    const stop = () => {
      if (timer !== null) window.clearTimeout(timer);
      timer = null;
      origin = null;
      setHold(null);
    };
    const onDown = (e: PointerEvent) => {
      stop();
      if (!isEmptySpot(e.target)) return;
      origin = { x: e.clientX, y: e.clientY };
      setHold(origin);
      timer = window.setTimeout(() => {
        stop();
        onOpen();
      }, MENU_HOLD_MS);
    };
    const onMove = (e: PointerEvent) => {
      if (origin && Math.hypot(e.clientX - origin.x, e.clientY - origin.y) > MENU_HOLD_SLOP_PX) stop();
    };
    window.addEventListener("pointerdown", onDown, true);
    window.addEventListener("pointermove", onMove, true);
    window.addEventListener("pointerup", stop, true);
    window.addEventListener("pointercancel", stop, true);
    return () => {
      stop();
      window.removeEventListener("pointerdown", onDown, true);
      window.removeEventListener("pointermove", onMove, true);
      window.removeEventListener("pointerup", stop, true);
      window.removeEventListener("pointercancel", stop, true);
    };
  }, [onOpen, enabled]);

  return hold;
}
