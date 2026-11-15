/** A swipe commits at 25% of the width or at more than 0.5 px/ms (DESIGN.md 3). */
export const SWIPE_DISTANCE_RATIO = 0.25;
export const SWIPE_SPEED_PX_MS = 0.5;
/** Horizontal movement the finger needs before the page follows it. */
export const SWIPE_START_PX = 12;

export type SwipeDecision = "next" | "prev" | "stay";

/**
 * What a released swipe does. `dx` is the horizontal movement (negative = left = next page), `vx` the speed at release
 * in px/ms (same sign convention), `width` the page width.
 */
export function swipeDecision(dx: number, vx: number, width: number): SwipeDecision {
  if (width <= 0 || dx === 0) return "stay";
  const far = Math.abs(dx) >= SWIPE_DISTANCE_RATIO * width;
  const fast = Math.abs(vx) > SWIPE_SPEED_PX_MS && Math.sign(vx) === Math.sign(dx);
  if (!far && !fast) return "stay";
  return dx < 0 ? "next" : "prev";
}

/** Page index after a decision; never leaves 0..count-1 (no wrap-around). */
export function targetPage(current: number, decision: SwipeDecision, count: number): number {
  if (decision === "next") return Math.min(current + 1, count - 1);
  if (decision === "prev") return Math.max(current - 1, 0);
  return current;
}

/** The finger moves the page 1:1 inside the range; beyond the first/last page it gives way at a third. */
export function dragOffset(dx: number, current: number, count: number): number {
  const beyond = (dx < 0 && current >= count - 1) || (dx > 0 && current <= 0);
  return beyond ? dx / 3 : dx;
}

/** Speed in px/ms from two samples; no time passed -> 0. */
export function velocity(x0: number, t0: number, x1: number, t1: number): number {
  return t1 > t0 ? (x1 - x0) / (t1 - t0) : 0;
}
