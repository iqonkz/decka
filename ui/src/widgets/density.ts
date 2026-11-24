/**
 * Responsive widget density helpers (DESIGN.md 4.16).
 * CSS uses `@container tile` breakpoints matching these dimensions so layout density responds to container size without JS.
 */
export type WidgetDensity = "S" | "M" | "L";

export const DENSITY_M_LONG = 220;
export const DENSITY_M_SQUARE = 140;
export const DENSITY_L_LONG = 220;
export const DENSITY_L_SHORT = 140;

/** Density bucket matching the `@container tile` pixel breakpoints. */
export function containerDensity(widthPx: number, heightPx: number): WidgetDensity {
  if (
    (widthPx >= DENSITY_L_LONG && heightPx >= DENSITY_L_SHORT) ||
    (heightPx >= DENSITY_L_LONG && widthPx >= DENSITY_L_SHORT)
  ) {
    return "L";
  }
  if (
    (widthPx >= DENSITY_M_SQUARE && heightPx >= DENSITY_M_SQUARE) ||
    widthPx >= DENSITY_M_LONG ||
    heightPx >= DENSITY_M_LONG
  ) {
    return "M";
  }
  return "S";
}

/** Cell-count density classification from DESIGN.md 4.16: S (<= 2), M (3..4 / 1-row), L (>= 6). */
export function widgetDensityByCells(w: number, h: number): WidgetDensity {
  const cells = w * h;
  if (cells <= 2) return "S";
  if (cells >= 6 && (w >= 2 && h >= 2)) return "L";
  if (cells >= 6) return "L";
  return "M";
}

/** Header appears on all widget panels with h >= 2 or w >= 3 (never on 1x1, 2x1, keys, context, or voice key). */
export function hasWidgetHeader(item: { kind: string; w: number; h: number }): boolean {
  if (item.kind === "button" || item.kind === "context" || item.kind === "voice") return false;
  return item.h >= 2 || item.w >= 3;
}
