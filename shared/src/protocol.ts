import type { ClientLayout } from "./layout";

// ===== Constants =====
export const DEFAULT_PORT = 9247;
export const PAIR_ALPHABET = "ABCDEFGHJKLMNPQRSTUVWXYZ23456789";
export const LIMITS = { title: 120, body: 500, source: 40, statusText: 80, group: 40, label: 60, notifications: 20 };

// ===== Base =====
export const PROTOCOL_VERSION = 1;
export type Id = string;        // ^[a-z0-9_-]{1,64}$
export type VarName = string;   // ^[a-z0-9_.-]{1,64}$
export type VarValue = string | number | boolean;

export type ErrorCode =
  | "UNAUTHORIZED" | "PAIR_CODE_INVALID" | "DEVICE_REVOKED" | "PROTOCOL_VERSION"
  | "BAD_MESSAGE" | "UNKNOWN_ACTION" | "ACTION_FAILED" | "RATE_LIMITED"
  | "PAYLOAD_TOO_LARGE" | "NOT_FOUND" | "INTERNAL";

// ===== Client -> Server (WS) =====
export interface ClientInfo { name: string; app: "capacitor" | "web"; version: string; }

export interface PairMsg  { t: "pair";  v: number; code: string; client: ClientInfo; }
export interface HelloMsg { t: "hello"; v: number; token: string; client: ClientInfo; }
export interface PressMsg { t: "press"; id: number; actionId: Id; }
export interface NotifDismissMsg { t: "notif_dismiss"; notifId: string; }
export interface PingMsg  { t: "ping"; ts: number; }
/** M16: the answer to a permission question of Claude Code (DATA_MODEL.md 6.8); `pc` = ask in the terminal instead. */
export interface PermAnswerMsg { t: "perm_answer"; reqId: string; decision: "allow" | "deny" | "pc"; }   // reqId ^[0-9a-f]{16}$
export type ClientMsg = PairMsg | HelloMsg | PressMsg | NotifDismissMsg | PingMsg | PermAnswerMsg;

// ===== Server -> Client (WS) =====
export interface PairedMsg  { t: "paired"; deviceId: Id; token: string; }
export interface WelcomeMsg {
  t: "welcome"; v: number;
  agent: { name: string; version: string };
  deviceId: Id; rev: number; state: FullState;
}
export interface PatchMsg<K extends SectionName = SectionName> {
  t: "patch"; rev: number; section: K; value: FullState[K];
}
export interface PressResultMsg { t: "press_result"; id: number; ok: boolean; error?: { code: ErrorCode; message: string }; }
export interface PongMsg  { t: "pong"; ts: number; }
export interface ErrorMsg { t: "error"; code: ErrorCode; message: string; fatal: boolean; supported?: number[]; }
export type ServerMsg = PairedMsg | WelcomeMsg | PatchMsg | PressResultMsg | PongMsg | ErrorMsg;

// ===== State (sections) =====
export type SectionName = "layout" | "sys" | "statuses" | "notifications" | "vars" | "focus" | "media" | "icons" | "weather" | "obs" | "ai" | "twitch"; // weather, obs: M15; ai: M16
export interface FullState {
  layout: ClientLayout;
  sys: SysStats | null;            // null = collector disabled
  statuses: StatusItem[];          // sorted by (group, id)
  notifications: Notification[];   // newest first, at most 20
  vars: Record<VarName, VarValue>;
  focus?: FocusInfo | null;        // M11; absent from agents older than M11; null = collector disabled
  media?: MediaInfo | null;        // M11; null = nothing playing or collector disabled
  icons?: Record<string, string>;  // M12: "<hash16>" -> "data:image/png;base64,..." (M13: or "data:image/svg+xml;base64,..."); only the images the layout refers to; absent = {}
  weather?: WeatherInfo | null;    // M15; null = off or no data yet
  obs?: ObsInfo | null;            // M15; null = the integration is off
  ai?: AiInfo | null;              // M16; null = both integrations are off
  twitch?: TwitchInfo | null;      // null = Twitch is off or nobody has signed in
}

/** The numbers of the signed-in Twitch channel. No token ever appears here. */
export interface TwitchInfo {
  connected: boolean;              // false = the last look at the channel failed (the numbers are the last good ones)
  login: string;                   // the channel name
  live: boolean;
  viewers: number;                 // 0 when not live
  followers: number;
  title?: string;                  // <= 140, while live
  game?: string;                   // <= 80, while live
  startedAt?: number;              // ts the stream began (the client counts the time on air)
}

// M16: the limits of Claude Code / Codex and the permission questions (DATA_MODEL.md 6.8).
export interface AiWindow {
  label: string;                   // "5h" | "7d" | "30d" | "<n>h", from the length of the window
  pct: number;                     // 0..100, one decimal
  resetsAt: number;                // ts the window resets (ms)
}
export interface AiProvider {
  windows: AiWindow[];             // 0..2, the short window first
  plan?: string;                   // Codex plan_type; Claude has none
  updatedAt: number;
}
export interface AiSession {
  id: string;                      // first 8 characters of session_id
  project: string;                 // basename(cwd), <= 60
  model?: string;                  // <= 40
  ctxPct?: number;                 // whole percent
  lastSeen: number;
}
export interface PermRequest {
  reqId: string;                   // 16 hex
  provider: "claude" | "codex";    // who asks
  tool: string;                    // <= 60
  summary: string;                 // what exactly (a command, a file, an address), <= 300
  project: string;                 // <= 60
  sessionId: string;
  expiresAt: number;               // ts after which the agent lets the question go to the terminal
}
export interface AiInfo {
  claude?: AiProvider | null;      // null = no data (not installed, or no session yet)
  codex?: AiProvider | null;       // null = off, or Codex not found
  sessions: AiSession[];           // Claude Code sessions seen in the last 10 min, newest first, <= 6
  pending: PermRequest[];          // waiting for an answer, oldest first, <= 5
}

// M15: weather (DATA_MODEL.md 6.3). Only the agent asks the internet; the phone never does.
export interface WeatherInfo {
  place: string;                   // <= 80, the place name from the settings
  units: "metric" | "imperial";
  updatedAt: number;               // ts the data was received
  current: { temp: number; code: number; isDay: boolean; wind: number };   // temp: whole degrees C/F; code: WMO; wind: whole km/h or mph
  daily: { date: string; min: number; max: number; code: number }[];      // exactly 3 days, date "YYYY-MM-DD", starting today
}

// M15: OBS Studio (DATA_MODEL.md 6.4). State only; the OBS password never reaches the protocol.
export interface ObsInfo {
  connected: boolean;
  recording: boolean;
  recordStartedAt?: number;        // ts the recording began (the client counts the timer)
  streaming: boolean;
  streamStartedAt?: number;
  scene?: string;                  // current scene, <= 100
  replayBuffer: boolean;           // the replay buffer is on (for a Save replay key)
}

// Active PC window: the executable name only, NEVER the window title.
export interface FocusInfo {
  app: string | null;              // lower-case exe name ("code.exe"); null = unknown (desktop, lock screen)
}

// Current Windows media session (Spotify, a browser, a player).
export interface MediaInfo {
  source: string;                  // app identifier from Windows (<= 120), for display only
  title: string;                   // <= 200
  artist?: string;                 // <= 200
  album?: string;                  // <= 200
  playing: boolean;
  positionSec?: number;            // integer; position at positionAt
  durationSec?: number;            // integer
  positionAt: number;              // ts the positionSec refers to
  art?: string;                    // "data:image/png;base64,..." or "data:image/jpeg;base64,...", <= 65536 chars
  shuffle?: boolean;               // M15: GSMTC IsShuffleActive; absent when the source has none
  repeat?: "none" | "list" | "track"; // M15: GSMTC AutoRepeatMode
}

export interface SysStats {
  cpu: number;                               // 0..100, integer
  mem: { usedMb: number; totalMb: number };  // integers
  disks: { mount: string; usedGb: number; totalGb: number }[]; // integers, fixed disks only
  net: { rxKbps: number; txKbps: number };   // integers, sum over all interfaces
  uptimeSec: number;                         // rounded down to 60
}

export type StatusState = "ok" | "warn" | "error" | "running" | "unknown";
export interface StatusItem {
  id: Id; label: string; state: StatusState;
  text?: string;                 // <= 80 chars
  group?: string;                // <= 40 chars
  source: "push" | "check";
  updatedAt: number;
}

export type NotifLevel = "info" | "success" | "warning" | "error";
export interface Notification {
  id: string;                    // decimal counter string: "1", "2", ...
  ts: number; title: string;     // title <= 120
  body?: string;                 // <= 500
  level: NotifLevel;
  source?: string;               // <= 40
}

// ===== HTTP =====
export interface HealthRes { name: "decka"; version: string; protocol: number; }
export interface NotifyReq { title: string; body?: string; level?: NotifLevel; source?: string; }
export interface NotifyRes { id: string; }
export interface StatusReq { id: Id; state: StatusState; label?: string; text?: string; group?: string; ttlSec?: number; }
export interface OkRes { ok: true; }
export interface VarReq { value: VarValue; }
export interface HttpError { error: { code: ErrorCode; message: string } }
