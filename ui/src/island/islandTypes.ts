// The event types are shared with the agent (`shared/src/island.ts`, T26.4); only the mapping to the sphere lives here.
import type { IslandEvent, OrbState } from "@decka/shared";

export type {
  IslandMode,
  OrbState,
  IslandVoicePhase,
  IslandActionKind,
  IslandSlowProgress,
  IslandContextChip,
  IslandVoiceState,
  IslandLyricsState,
  IslandMediaKind,
  IslandEvent,
} from "@decka/shared";
/** What the compact island shows for a song, derived from the shared event (no second protocol): see `IslandEvent`. */
export interface SongView {
  playing: boolean;
  current?: string;
  next?: string;
  musicLevel?: number;
  muted: boolean;
  title?: string;
  artist?: string;
  /** Seconds until the first line; `null` when the time is unknown. */
  nextInSec?: number | null;
  lyricsPending?: boolean;
}

/** The song of an event: `null` when no track is known and no lyrics are shown. */
export function songView(event: IslandEvent): SongView | null {
  const { lyrics, track } = event;
  if (!lyrics && !track && !event.playing) return null;
  const ms = lyrics?.firstLineInMs;
  return {
    playing: Boolean(event.playing ?? lyrics?.playing),
    current: lyrics?.current,
    next: lyrics?.next,
    musicLevel: event.musicLevel,
    muted: Boolean(event.muted),
    title: track?.title,
    artist: track?.artist,
    nextInSec: typeof ms === "number" ? ms / 1000 : undefined,
    lyricsPending: lyrics?.status === "loading",
  };
}

/** The words of the compact island when it shows no lyrics: what was heard or answered; "Listening..." only while the microphone records. */
export function compactVoiceText(voice: IslandEvent["voice"], listeningLabel: string): string {
  if (!voice) return "";
  const said = voice.reply ?? voice.heard;
  if (said) return said;
  return voice.active && voice.phase === "listening" ? listeningLabel : "";
}

/**
 * Maps an island state snapshot into the orb's visual state + error flag (TASKS.md §T26.5b, §T26.F.11).
 * - loading / starting server -> `connecting`
 * - listening -> `listening`
 * - waiting on model or slow path -> `thinking`
 * - speaking (or playing song when voice is inactive) -> `speaking`
 * - error -> `idle` with `isError: true` (60% desaturation + 200 ms shake wrapper)
 */
export function orbStateFromIsland(event: IslandEvent): { orbState: OrbState; isError: boolean } {
  const voice = event.voice;
  if (voice) {
    if (voice.phase === "error") return { orbState: "idle", isError: true };
    if (voice.phase === "loading") return { orbState: "connecting", isError: false };
    // T26.F.12: "listening" only while the microphone really records (the voice host is active), never for a song without a line.
    if (voice.phase === "listening" && voice.active) return { orbState: "listening", isError: false };
    if (voice.phase === "recognizing" || voice.phase === "thinking" || voice.slow) {
      return { orbState: "thinking", isError: false };
    }
    if (voice.phase === "speaking") return { orbState: "speaking", isError: false };
  }
  // A playing song moves the orb like speech (T26.F.11), driven by `musicLevel`.
  if (!voice?.active && (event.playing ?? event.lyrics?.playing)) {
    return { orbState: "speaking", isError: false };
  }
  return { orbState: "idle", isError: false };
}
