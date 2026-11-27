// T26.4: the "island" of the PC (TASKS.md M26) - one window at the top of the main monitor for the voice assistant and the lyrics.
// The agent decides the mode (a pure state machine in decka-core) and sends `island_state`; the page only draws it and reports the
// rectangle of its visible panel (`island_rect`) so that the rest of the window lets clicks through.

export type IslandMode = "hidden" | "tab" | "compact" | "expanded" | "orb";

/** T29.5: how the island rests while a song plays (`config.island`). */
export interface IslandSettings {
  restingMode: "bar" | "lyrics";
  /** 6..16. */
  barHeight: number;
  barPosition: "left" | "center" | "right";
}

/** T29.4: the orb of a dictation at the text cursor (`mode: "orb"`); `text` / `key`: the bubble with what went wrong. */
export interface IslandOrbState {
  phase: "listening" | "thinking" | "done" | "error";
  level?: number;
  text?: string | null;
  key?: string | null;
}

/** The states of the sphere (`ParticlesOrb`). */
export type OrbState = "idle" | "connecting" | "listening" | "thinking" | "speaking";

export type IslandVoicePhase = "idle" | "loading" | "listening" | "recognizing" | "thinking" | "speaking" | "confirm" | "done" | "error";

/** What a button under the shown answer asks the PC to do (`voice_action`); the PC decides what it means. */
export type IslandActionKind = "copy" | "insert" | "speak" | "again" | "memory" | `lang:${string}`;

/** The slow path (vision / 9B model): the estimate and the time so far, in whole seconds. Longer than 2 x etaSec: "A little longer...". */
export interface IslandSlowProgress {
  etaSec: number;
  elapsedSec?: number;
}

/** A hint chip (T26.7): `command` is the same request as if it were said. */
export interface IslandContextChip {
  label: string;
  command: string;
}

export interface IslandVoiceState {
  active: boolean;
  phase: IslandVoicePhase;
  /** Microphone level 0..1. */
  level?: number;
  heard?: string;
  reply?: string;
  /** Buttons under the reply: `copy`, `insert`, `speak`, `again`, `lang` (the list of languages). */
  actions?: string[];
  slow?: IslandSlowProgress | null;
  /** T29.2: the Screen help package is being downloaded for this question. */
  download?: { pct: number } | null;
  /** T29.A: the model is being loaded into the memory; the seconds that are probably left. */
  loading?: { etaSec: number } | null;
  chips?: IslandContextChip[];
  confirmId?: string;
  /** A fixed phrase of the agent as an i18n key with its arguments (`voice.reply.*`, `voice.status.*`); `reply` is the fallback text. */
  replyKey?: string;
  replyArgs?: Record<string, string>;
  /** What the confirmation asks (`phase: "confirm"`): free text or an i18n key (`voice.confirm.*`) with its arguments. */
  confirm?: { text: string; key?: string; args?: Record<string, string> };
  /** T28.3: what the phrase was understood as (a command that is being run or was run). */
  understood?: { text: string; key?: string; args?: Record<string, string> };
  /** T28.3: the understood command is being run now. */
  executing?: boolean;
}

export interface IslandLyricsState {
  playing: boolean;
  /** T28.5b: the sung line before the current one; absent up to the first line. */
  prev?: string;
  current?: string;
  next?: string;
  /** T26.F.13: `loading` while the lyrics are being searched for (the island shows three dots), `ready` with lines, `none` when not found or off. */
  status?: "loading" | "ready" | "none";
  /** T26.F.10: milliseconds until the first sung line of a synced song; absent/null once it has started or when the time is unknown. */
  firstLineInMs?: number | null;
}

/** The media buttons of the island (`media_control { kind }`): the same operations as the deck buttons; the PC refuses any other name. */
export type IslandMediaKind = "prev" | "play_pause" | "next" | "mute";

/** The event `island_state` (agent -> window). */
export interface IslandEvent {
  mode: IslandMode;
  /** Only in `mode: "orb"`. */
  orb?: IslandOrbState | null;
  /** T29.5: the height and the side of the resting bar (`mode: "tab"`). */
  barHeight?: number;
  barPosition?: "left" | "center" | "right";
  lyrics?: IslandLyricsState | null;
  voice?: IslandVoiceState | null;
  /** T26.F.10: the track on the PC (title and artist from the media section); absent when nothing is loaded. */
  track?: { title: string; artist?: string };
  /** T26.F.10: a song is playing (with or without a lyrics line). */
  playing?: boolean;
  /** The output of the PC is muted. */
  muted?: boolean;
  /** Peak level of the PC output 0..1 (20 times a second, only while a song plays and the island is open). */
  musicLevel?: number;
  /** T30.1: a meeting note is being recorded; the island shows it in every state. */
  recording?: { active: boolean; elapsedSec: number } | null;
  /** T30.2: the programs that use the most memory, with a "Close" button each. */
  memory?: { pid: number; name: string; mb: number }[] | null;
}

/** The IPC `island_rect` (window -> agent): the visible panel, in CSS pixels from the top-left corner of the window. */
export interface IslandRect {
  x: number;
  y: number;
  width: number;
  height: number;
}
