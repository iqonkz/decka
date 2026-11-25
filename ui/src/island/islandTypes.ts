export type OrbState = "idle" | "connecting" | "listening" | "thinking" | "speaking";

export type IslandMode = "hidden" | "tab" | "compact" | "expanded";

export type IslandVoicePhase =
  | "idle"
  | "loading"
  | "listening"
  | "recognizing"
  | "thinking"
  | "speaking"
  | "confirm"
  | "done"
  | "error";

export type IslandActionKind = "copy" | "insert" | "speak" | "again" | `lang:${string}`;

export interface IslandSlowProgress {
  /** Moving average ETA in seconds (default 10 s for 4B, 20 s for 9B). */
  etaSec: number;
  /** Elapsed seconds so far (> 2 * etaSec switches label to "Almost there..."). */
  elapsedSec?: number;
}

export interface IslandContextChip {
  label: string;
  command: string;
}

export interface IslandVoiceState {
  active: boolean;
  phase: IslandVoicePhase;
  /** Normalized microphone / audio energy 0..1. */
  level?: number;
  heard?: string;
  reply?: string;
  /** Action buttons shown below the reply (`copy`, `insert`, `speak`, `again`, `lang:<code>`). */
  actions?: string[];
  slow?: IslandSlowProgress | null;
  chips?: IslandContextChip[];
  confirmId?: string;
}

export interface IslandLyricsState {
  playing: boolean;
  current?: string;
  next?: string;
}

export interface IslandEvent {
  mode: IslandMode;
  lyrics?: IslandLyricsState | null;
  voice?: IslandVoiceState | null;
}

/**
 * Maps an island state snapshot into the orb's visual state + error flag (TASKS.md §T26.5b).
 * - loading / starting server -> `connecting`
 * - listening -> `listening`
 * - waiting on model or slow path -> `thinking`
 * - speaking -> `speaking`
 * - error -> `idle` with `isError: true` (60% desaturation + 200 ms shake wrapper)
 */
export function orbStateFromIsland(event: IslandEvent): { orbState: OrbState; isError: boolean } {
  const voice = event.voice;
  if (!voice) return { orbState: "idle", isError: false };
  if (voice.phase === "error") return { orbState: "idle", isError: true };
  if (voice.phase === "loading") return { orbState: "connecting", isError: false };
  if (voice.phase === "listening") return { orbState: "listening", isError: false };
  if (voice.phase === "recognizing" || voice.phase === "thinking" || voice.slow) {
    return { orbState: "thinking", isError: false };
  }
  if (voice.phase === "speaking") return { orbState: "speaking", isError: false };
  return { orbState: "idle", isError: false };
}
