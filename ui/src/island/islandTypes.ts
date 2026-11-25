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
  IslandEvent,
} from "@decka/shared";
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
