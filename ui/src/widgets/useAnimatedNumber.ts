import { useEffect, useRef, useState } from "react";

const isMotionReduced = (): boolean => {
  if (typeof document === "undefined") return true;
  if (document.documentElement.dataset.reduced === "1") return true;
  return typeof window !== "undefined" && window.matchMedia?.("(prefers-reduced-motion: reduce)").matches === true;
};

/**
 * Smoothly counts between numeric updates over `durationMs` (DESIGN.md 4.16: 300 ms for % and temp).
 * Initial render / SSR immediately shows `target`; reduced motion snaps without animation.
 */
export function useAnimatedNumber(target: number, durationMs = 300, decimals = 0): number {
  const [shown, setShown] = useState(target);
  const prevRef = useRef(target);
  const shownRef = useRef(target);
  shownRef.current = shown;

  useEffect(() => {
    if (!Number.isFinite(target)) {
      prevRef.current = target;
      setShown(target);
      return;
    }
    const from = shownRef.current;
    if (from === target || isMotionReduced() || typeof requestAnimationFrame !== "function") {
      prevRef.current = target;
      setShown(target);
      return;
    }
    const factor = 10 ** decimals;
    const start = performance.now();
    let raf = 0;
    const step = (now: number) => {
      const p = Math.min(1, (now - start) / durationMs);
      const eased = 1 - (1 - p) ** 2;
      const val = from + (target - from) * eased;
      const rounded = Math.round(val * factor) / factor;
      setShown(p >= 1 ? target : rounded);
      if (p < 1) {
        raf = requestAnimationFrame(step);
      } else {
        prevRef.current = target;
      }
    };
    raf = requestAnimationFrame(step);
    return () => cancelAnimationFrame(raf);
  }, [target, durationMs, decimals]);

  return shown;
}
