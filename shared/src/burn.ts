/** One reading of a limit window: when it was seen and how full it was. */
export interface BurnSample {
  ts: number;
  pct: number;
}

/** At most this many points per window, from the last hour (DATA_MODEL.md 6.8). */
export const BURN_MAX_SAMPLES = 60;
export const BURN_WINDOW_MS = 60 * 60 * 1000;
const MIN_POINTS = 3;
const MIN_SPAN_MS = 10 * 60 * 1000;
const MIN_GROWTH = 1;

/** One point a minute at most: readings that come faster only refresh the last point. */
const MIN_STEP_MS = 60 * 1000;

/** Adds a reading; a fall of the percentage (the window was reset) starts the history again. Returns the new list. */
export function pushBurnSample(samples: readonly BurnSample[], sample: BurnSample): BurnSample[] {
  const last = samples[samples.length - 1];
  const kept = last !== undefined && sample.pct < last.pct ? [] : samples.filter((s) => sample.ts - s.ts <= BURN_WINDOW_MS);
  const tail = kept[kept.length - 1];
  if (tail !== undefined && sample.ts - tail.ts < MIN_STEP_MS) return [...kept.slice(0, -1), { ts: tail.ts, pct: sample.pct }];
  return [...kept, sample].slice(-BURN_MAX_SAMPLES);
}

/**
 * When the window will be full at the speed seen so far, or `null` when there is no forecast: fewer than 3 points, less than
 * 10 minutes between the first and the last, a growth under 1 point, or the window resets first. Speed = first and last point.
 */
export function burnEta(samples: readonly BurnSample[], now: number, resetsAt: number): number | null {
  if (samples.length < MIN_POINTS) return null;
  const first = samples[0]!;
  const last = samples[samples.length - 1]!;
  const span = last.ts - first.ts;
  const growth = last.pct - first.pct;
  if (span < MIN_SPAN_MS || growth < MIN_GROWTH) return null;
  const perMs = growth / span;
  const eta = now + (100 - last.pct) / perMs;
  return eta < resetsAt ? eta : null;
}

/** `2h 14m`, `14m` (under an hour) or `3d 4h` (over a day); the unit letters come from the caller (they are translated). */
export function formatLeft(ms: number, units: { d: string; h: string; m: string }): string {
  const minutes = Math.max(0, Math.floor(ms / 60000));
  if (minutes >= 24 * 60) return `${Math.floor(minutes / 1440)}${units.d} ${Math.floor((minutes % 1440) / 60)}${units.h}`;
  if (minutes >= 60) return `${Math.floor(minutes / 60)}${units.h} ${minutes % 60}${units.m}`;
  return `${minutes}${units.m}`;
}
