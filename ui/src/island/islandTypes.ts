// The event types are shared with the agent (`shared/src/island.ts`, T26.4); only the mapping to the sphere lives here.
import type { IslandEvent, IslandLyricsState, OrbState } from "@decka/shared";

export type {
  IslandMode,
  OrbState,
  IslandVoicePhase,
  IslandActionKind,
  IslandSlowProgress,
  IslandContextChip,
  IslandVoiceState,
  IslandLyricsState,
  IslandEvent,
} from "@decka/shared";

/** Local T26.F.11-12 extensions over shared island types until synced in shared/src/island.ts. */
export type IslandMediaAction = "prev" | "play_pause" | "next" | "mute";

export interface IslandMusicLyricsState extends IslandLyricsState {
  /** Normalized 0..1 audio energy of the currently playing track. */
  musicLevel?: number;
  /** Whether playback audio is muted. */
  muted?: boolean;
  /** Track title shown when no active lyric line is available. */
  title?: string;
  /** Artist name shown when no active lyric line is available. */
  artist?: string;
  /** Seconds until the next/first lyric line (`null` when timing is unknown, e.g. syncing). */
  nextInSec?: number | null;
  /** True while waiting for upcoming/unsynced timing without a countdown. */
  lyricsPending?: boolean;
}

export interface IslandMusicEvent extends Omit<IslandEvent, "lyrics"> {
  lyrics?: IslandMusicLyricsState | null;
}

/**
 * Maps an island state snapshot into the orb's visual state + error flag (TASKS.md §T26.5b, §T26.F.11).
 * - loading / starting server -> `connecting`
 * - listening -> `listening`
 * - waiting on model or slow path -> `thinking`
 * - speaking (or playing song when voice is inactive) -> `speaking`
 * - error -> `idle` with `isError: true` (60% desaturation + 200 ms shake wrapper)
 */
export function orbStateFromIsland(event: IslandMusicEvent): { orbState: OrbState; isError: boolean } {
  const voice = event.voice;
  if (voice) {
    if (voice.phase === "error") return { orbState: "idle", isError: true };
    if (voice.phase === "loading") return { orbState: "connecting", isError: false };
    if (voice.phase === "listening") return { orbState: "listening", isError: false };
    if (voice.phase === "recognizing" || voice.phase === "thinking" || voice.slow) {
      return { orbState: "thinking", isError: false };
    }
    if (voice.phase === "speaking") return { orbState: "speaking", isError: false };
  }
  if (!voice?.active && event.lyrics?.playing) {
    return { orbState: "speaking", isError: false };
  }
  return { orbState: "idle", isError: false };
}
