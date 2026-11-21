import type { AiWindow } from "@decka/shared";

/** Pure rules of the AI limits widget (DESIGN.md 4.11): colours, short times, which window and how much text fits. */

export type Units = { d: string; h: string; m: string };

/** Level colour of a ring or bar: under 70% the accent, 70..89% `warning`, from 90% `error` (DESIGN.md 4.11). */
export type Level = "accent" | "warning" | "error";
export function levelOf(pct: number): Level {
  if (pct >= 90) return "error";
  if (pct >= 70) return "warning";
  return "accent";
}

/** The shortest time to a reset: `2h 14m`, `3d` (a day or more), `14m` (under an hour). The unit letters are translated by the caller. */
export function formatReset(ms: number, units: Units): string {
  const minutes = Math.max(0, Math.floor(ms / 60000));
  if (minutes >= 24 * 60) return `${Math.floor(minutes / 1440)}${units.d}`;
  if (minutes >= 60) return `${Math.floor(minutes / 60)}${units.h} ${minutes % 60}${units.m}`;
  return `${minutes}${units.m}`;
}

/** `5h` -> `5ч` with the translated unit; a label that is not `<n>h` / `<n>d` stays as it is. */
export function windowLabel(label: string, units: Units): string {
  const m = /^(\d+)([hd])$/.exec(label);
  return m ? `${m[1]}${m[2] === "d" ? units.d : units.h}` : label;
}

/** The window that is closest to its limit (the highest percent; the shorter window wins a tie), or null without windows. */
export function closestWindow(windows: readonly AiWindow[]): AiWindow | null {
  let best: AiWindow | null = null;
  for (const w of windows) if (best === null || w.pct > best.pct) best = w;
  return best;
}

export type AiMode = "rings1" | "rings2" | "rows";

/** 2x1 (and any single row narrower than 3): rings side by side; 2x2: concentric rings; from 3x2: a row per provider with two bars. */
export function aiMode(w: number, h: number): AiMode {
  if (h < 2) return "rings1";
  return w >= 3 ? "rows" : "rings2";
}

/** Narrowest text of the widget, px (DESIGN.md 4.11: not smaller than 11). */
export const MIN_TEXT = 11;

/** Rough width of `text` set in `px` (digits and the symbols of the widget are about 0.62 em wide). */
export function textWidth(text: string, px: number): number {
  return Math.ceil([...text].length * px * 0.62);
}

/** What a provider block of the `rows` mode may show in `width` x `height` px; things that do not fit are dropped, never cut. */
export function rowsFit(width: number, height: number, hasSecondWindow: boolean, resetText: string): { labels: boolean; reset: boolean; bars: number } {
  const lines = hasSecondWindow ? 2 : 1;
  const barsHeight = lines * 13 + (lines - 1) * 4;
  const resetHeight = 14 + 4;
  const reset = height >= barsHeight + resetHeight && textWidth(resetText, 12) <= width - 24;
  const barsBase = width - 18 - 8 - 40; // brand 18 px, gap, the percent column
  const labels = barsBase - 26 >= 56;
  return { labels, reset, bars: lines };
}

/** The 5h-style ring geometry: stroke `stroke`, outer diameter `d`; returns the radius of the centre line and its length. */
export function ringGeometry(d: number, stroke: number, inset = 0): { r: number; c: number } {
  const r = d / 2 - stroke / 2 - inset;
  return { r, c: 2 * Math.PI * r };
}
