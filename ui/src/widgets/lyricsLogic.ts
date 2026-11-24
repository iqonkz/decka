import type { MediaInfo } from "@decka/shared";
import type { LyricsLine } from "./m24Types";

/**
 * Computes current track position in milliseconds with offset applied (DATA_MODEL.md 14).
 * Advances elapsed time only when playing; stays fixed at `positionSec * 1000 + offsetMs` when paused.
 */
export function effectiveMediaPositionMs(
  media: Pick<MediaInfo, "playing" | "positionSec" | "positionAt">,
  nowMs: number,
  offsetMs = 0,
): number {
  const baseMs = (media.positionSec ?? 0) * 1000;
  const elapsedMs = media.playing ? Math.max(0, nowMs - media.positionAt) : 0;
  return baseMs + elapsedMs + offsetMs;
}

/**
 * Selects the active synced lyric index: the last line whose `t <= effectivePositionMs`.
 * Returns -1 before the first timestamped line or if no timestamped lines exist.
 */
export function selectActiveLyricIndex(lines: readonly LyricsLine[], effectivePositionMs: number): number {
  let active = -1;
  for (let i = 0; i < lines.length; i++) {
    const lineT = lines[i]?.t;
    if (lineT === undefined) continue;
    if (lineT <= effectivePositionMs) active = i;
    else if (lineT > effectivePositionMs && active >= 0) break;
  }
  return active;
}

/** Formats signed offset seconds (`+1.0`, `0.0`, `−0.5`) for display in the lyrics offset bar. */
export function formatOffsetSeconds(offsetMs: number): string {
  const sec = offsetMs / 1000;
  const abs = Math.abs(sec).toFixed(1);
  if (offsetMs < 0) return `−${abs}`;
  if (offsetMs > 0) return `+${abs}`;
  return abs;
}
