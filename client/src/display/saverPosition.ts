const MARGIN_PX = 16;

/** Small deterministic generator (mulberry32): the same seed always gives the same number in [0, 1). */
function random(seed: number): number {
  let t = (Math.imul(seed | 0, 0x9e3779b1) + 0x6d2b79f5) | 0;
  t = Math.imul(t ^ (t >>> 15), t | 1);
  t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
  return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
}

export interface Size {
  w: number;
  h: number;
}

/**
 * Top-left corner of the screen-saver block for a 30 s interval number `seed`. The block lies fully
 * inside the viewport with a 16 px margin (when it cannot fit, it sits at the margin).
 */
export function saverPosition(seed: number, viewport: Size, box: Size): { x: number; y: number } {
  const freeW = Math.max(0, viewport.w - box.w - 2 * MARGIN_PX);
  const freeH = Math.max(0, viewport.h - box.h - 2 * MARGIN_PX);
  // Two independent draws per seed.
  return {
    x: Math.round(MARGIN_PX + random(seed * 2) * freeW),
    y: Math.round(MARGIN_PX + random(seed * 2 + 1) * freeH),
  };
}
