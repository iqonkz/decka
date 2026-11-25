// T26.F.14: the width of the compact island with a song line: by the longer of the two lines, 360..520 px.

export const COMPACT_MIN = 360;
export const COMPACT_MAX = 520;
/** Orb (32) + gap (12) + padding (12 + 14) + a little room for rounding. */
const CHROME = 32 + 12 + 12 + 14 + 6;

export type Measure = (text: string, font: string) => number;

let ctx: CanvasRenderingContext2D | null | undefined;

/** Browser measure with a rough fallback where there is no canvas (tests, server rendering). */
export const measureText: Measure = (text, font) => {
  if (ctx === undefined) {
    try {
      ctx = typeof document === "undefined" ? null : document.createElement("canvas").getContext("2d");
    } catch {
      ctx = null;
    }
  }
  if (!ctx) return text.length * (font.includes("14px") ? 7.6 : 6.6);
  ctx.font = font;
  return ctx.measureText(text).width;
};

/** The panel width for the two lines, clamped to the 360..520 range. */
export function compactWidth(current: string | undefined, next: string | undefined, measure: Measure = measureText): number {
  const a = current ? measure(current, '600 14px "Segoe UI Variable", system-ui, sans-serif') : 0;
  const b = next ? measure(next, '500 12px "Segoe UI Variable", system-ui, sans-serif') : 0;
  return Math.min(COMPACT_MAX, Math.max(COMPACT_MIN, Math.ceil(Math.max(a, b)) + CHROME));
}

/** Whether the current line is longer than the room of a panel of the widest size (then it wraps onto a second line). */
export function currentWraps(current: string | undefined, measure: Measure = measureText): boolean {
  return Boolean(current) && measure(current!, '600 14px "Segoe UI Variable", system-ui, sans-serif') + CHROME > COMPACT_MAX;
}
