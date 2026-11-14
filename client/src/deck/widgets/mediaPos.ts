import type { MediaInfo } from "@decka/shared";

/**
 * Where the track is now (whole seconds), by the formula of PROTOCOL.md 4.5: `positionSec + (now - positionAt) / 1000`,
 * no further than `durationSec`. A paused track stays at `positionSec`; no `positionSec` means unknown (`null`).
 */
export function currentPosition(media: MediaInfo, now: number): number | null {
  if (media.positionSec === undefined) return null;
  if (!media.playing) return media.positionSec;
  const moved = media.positionSec + Math.max(0, Math.floor((now - media.positionAt) / 1000));
  return media.durationSec !== undefined ? Math.min(moved, media.durationSec) : moved;
}

/** `m:ss`, from an hour on `h:mm:ss`. */
export function formatTime(sec: number): string {
  const total = Math.max(0, Math.floor(sec));
  const h = Math.floor(total / 3600);
  const m = Math.floor((total % 3600) / 60);
  const s = String(total % 60).padStart(2, "0");
  return h > 0 ? `${h}:${String(m).padStart(2, "0")}:${s}` : `${m}:${s}`;
}
