/** The Windows volume key moves the volume by this many percent (DATA_MODEL.md 6.6). */
export const VOLUME_STEP = 2;
/** At most this many presses a second while a slider is dragged. */
export const VOLUME_PRESSES_PER_SECOND = 15;
/** After the finger lifts, the slider shows the value of `audio.volume` again once this long has passed. */
export const VOLUME_SETTLE_MS = 600;

/** The key presses that bring the volume from `fromPct` to `toPct`: the direction and how many 2 % steps. */
export function volumeSteps(fromPct: number, toPct: number): { action: "up" | "down"; count: number } {
  const diff = toPct - fromPct;
  return { action: diff < 0 ? "down" : "up", count: Math.floor(Math.abs(diff) / VOLUME_STEP) };
}

/** The value 0..100 under the finger: `pos` and `start` in px along the track of `length` px; a vertical track grows upwards. */
export function volumeAt(pos: number, start: number, length: number, vertical: boolean): number {
  if (length <= 0) return 0;
  const fraction = (pos - start) / length;
  const value = Math.round((vertical ? 1 - fraction : fraction) * 100);
  return Math.min(100, Math.max(0, value));
}
