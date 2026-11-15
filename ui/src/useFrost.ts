import { useEffect } from "react";
import { frostPosition } from "./wallpaper";

const SELECTOR = ".key, .tile.panel";

/** Horizontal and vertical translation of the nearest page layer (a swipe or a slide-in in progress), 0 if none. */
function layerShift(el: Element): { x: number; y: number } {
  const layer = el.closest(".page-layer");
  if (!layer) return { x: 0, y: 0 };
  const m = getComputedStyle(layer).transform;
  if (!m || m === "none") return { x: 0, y: 0 };
  try {
    const matrix = new DOMMatrix(m);
    return { x: matrix.m41, y: matrix.m42 };
  } catch {
    return { x: 0, y: 0 };
  }
}

/**
 * `reduced` Glass (DESIGN.md 1.4): gives every key and widget panel the position of the frosted picture inside it, so that
 * the picture shows the piece of wallpaper right under the key (`frostPosition`). Positions are recomputed when the DOM
 * of the deck changes (a page, a layout, a widget) and when the screen is resized; nothing runs per frame.
 */
export function useFrost(enabled: boolean): void {
  useEffect(() => {
    if (!enabled) return;
    let frame = 0;
    const apply = () => {
      frame = 0;
      const viewport = { width: window.innerWidth, height: window.innerHeight };
      for (const el of Array.from(document.querySelectorAll<HTMLElement>(SELECTOR))) {
        const rect = el.getBoundingClientRect();
        const shift = layerShift(el);
        const p = frostPosition({ left: rect.left - shift.x, top: rect.top - shift.y, width: rect.width, height: rect.height }, viewport);
        el.style.setProperty("--frost-pos", `${p.x}px ${p.y}px`);
        el.style.setProperty("--frost-size", `${p.width}px ${p.height}px`);
      }
    };
    const schedule = () => {
      if (!frame) frame = requestAnimationFrame(apply);
    };
    const observer = new MutationObserver(schedule);
    observer.observe(document.body, { childList: true, subtree: true });
    window.addEventListener("resize", schedule);
    schedule();
    return () => {
      observer.disconnect();
      window.removeEventListener("resize", schedule);
      if (frame) cancelAnimationFrame(frame);
      for (const el of Array.from(document.querySelectorAll<HTMLElement>(SELECTOR))) {
        el.style.removeProperty("--frost-pos");
        el.style.removeProperty("--frost-size");
      }
    };
  }, [enabled]);
}
