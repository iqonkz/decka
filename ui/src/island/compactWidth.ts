// T26.F.14: compact island with a song is fixed at 520 px.

export const COMPACT_MIN = 520;
export const COMPACT_MAX = 520;
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
  if (!ctx) return text.length * (font.includes("14px") ? 7.6 : 6.6);
  ctx.font = font;
  return ctx.measureText(text).width;
};

/** Constant 520 px panel width for compact song mode (T26.F.14). */
export function compactWidth(_current?: string, _next?: string, _measure?: Measure): number {
  return COMPACT_MAX;
}

/** Whether the current line exceeds the text column of the 520 px panel (wraps up to 3 lines). */
export function currentWraps(current: string | undefined, measure: Measure = measureText): boolean {
  return Boolean(current) && measure(current!, '600 14px "Segoe UI Variable", system-ui, sans-serif') + CHROME > COMPACT_MAX;
}
