// T28.5b: compact island with a song is fixed at 440 px; current wraps onto lines 2-3 if wider.

export const COMPACT_MIN = 440;
export const COMPACT_MAX = 440;
/** Orb col (48) + gap (14) + padding (12 + 12) + rounding room. */
const CHROME = 48 + 14 + 12 + 12 + 6;

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
  if (!ctx) return text.length * (font.includes("16px") ? 8.8 : 7.6);
  ctx.font = font;
  return ctx.measureText(text).width;
};

/** Constant 440 px panel width for compact song mode (T28.5b). */
export function compactWidth(_current?: string, _next?: string, _measure?: Measure): number {
  return COMPACT_MAX;
}

/** Whether the current line exceeds one line of the 440 px panel's text column (then occupies lines 2-3). */
export function currentWraps(current: string | undefined, measure: Measure = measureText): boolean {
  return Boolean(current) && measure(current!, '600 16px "Segoe UI Variable", system-ui, sans-serif') + CHROME > COMPACT_MAX;
}
