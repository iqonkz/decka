import type { DisplaySettings } from "@decka/shared";

export type ScreenPreset = "off" | "balanced" | "maximum" | "custom";

/** The numbers behind each named choice (DESIGN.md 4.6a). `balanced` is the default of a new install. */
export const PRESETS: Record<Exclude<ScreenPreset, "custom">, { dimAfterSec: number; dimLevel: number; sleepAfterSec: number }> = {
  off: { dimAfterSec: 0, dimLevel: 0.3, sleepAfterSec: 0 },
  balanced: { dimAfterSec: 60, dimLevel: 0.3, sleepAfterSec: 600 },
  maximum: { dimAfterSec: 30, dimLevel: 0.15, sleepAfterSec: 180 },
};

export const PRESET_LABELS: Record<ScreenPreset, string> = { off: "Off", balanced: "Balanced", maximum: "Maximum", custom: "Custom" };

const same = (a: number, b: number) => Math.abs(a - b) < 1e-9;

/** Which choice the numbers match; anything else is `custom`. `off` ignores the dim level (nothing dims then). */
export function screenPreset(d: Pick<DisplaySettings, "dimAfterSec" | "dimLevel" | "sleepAfterSec">): ScreenPreset {
  if (d.dimAfterSec === 0 && d.sleepAfterSec === 0) return "off";
  for (const name of ["balanced", "maximum"] as const) {
    const p = PRESETS[name];
    if (d.dimAfterSec === p.dimAfterSec && same(d.dimLevel, p.dimLevel) && d.sleepAfterSec === p.sleepAfterSec) return name;
  }
  return "custom";
}

/** The display with the numbers of a named choice; `custom` leaves them as they are. */
export function applyScreenPreset<D extends DisplaySettings>(display: D, preset: ScreenPreset): D {
  return preset === "custom" ? display : { ...display, ...PRESETS[preset] };
}

/** `90` s -> `1.5` minutes for the `Custom` fields; `0` stays `0`. */
export const secToMin = (sec: number): number => Math.round((sec / 60) * 100) / 100;
export const minToSec = (min: number): number => Math.round(min * 60);
/** The brightness fraction (0.3) as a percent (30) and back. */
export const fracToPercent = (f: number): number => Math.round(f * 100);
export const percentToFrac = (p: number): number => Math.round(p) / 100;