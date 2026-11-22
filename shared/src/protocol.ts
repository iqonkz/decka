import type { ClientLayout } from "./layout";

// ===== Constants =====
export const DEFAULT_PORT = 9247;
export const PAIR_ALPHABET = "ABCDEFGHJKLMNPQRSTUVWXYZ23456789";
export const LIMITS = { title: 120, body: 500, source: 40, statusText: 80, group: 40, label: 60, notifications: 20 };

// ===== Base =====
export const PROTOCOL_VERSION = 2;
export type Id = string;        // ^[a-z0-9_-]{1,64}$
export type VarName = string;   // ^[a-z0-9_.-]{1,64}$
export type VarValue = string | number | boolean;

export type ErrorCode =
  | "UNAUTHORIZED" | "PAIR_CODE_INVALID" | "DEVICE_REVOKED" | "PROTOCOL_VERSION"
  | "BAD_MESSAGE" | "UNKNOWN_ACTION" | "ACTION_FAILED" | "RATE_LIMITED"
  | "PAYLOAD_TOO_LARGE" | "NOT_FOUND" | "INTERNAL";

// ===== Client -> Server (WS) =====
export interface ClientInfo { name: string; app: "capacitor" | "web"; version: string; }

/** M18 (PROTOCOL.md 10): the first frame of protocol v2. `cn` is 16 random bytes, base64url. */
export interface Hello2Msg { t: "hello2"; v: number; mode: "device" | "pair_qr" | "pair_code"; deviceId?: Id; cn: string; client: ClientInfo; }
/** M18: the proof of the client, HMAC-SHA256 in base64url. */
export interface AuthMsg { t: "auth"; mac: string; }
/** v1: plain text, accepted from loopback only. */
export interface PairMsg  { t: "pair";  v: number; code: string; client: ClientInfo; }
export interface HelloMsg { t: "hello"; v: number; token: string; client: ClientInfo; }
export interface PressMsg { t: "press"; id: number; actionId: Id; }
export interface NotifDismissMsg { t: "notif_dismiss"; notifId: string; }
export interface PingMsg  { t: "ping"; ts: number; }
/** M16: the answer to a permission question of Claude Code (DATA_MODEL.md 6.8); `pc` = ask in the terminal instead. */
export interface PermAnswerMsg { t: "perm_answer"; reqId: string; decision: "allow" | "allow_always" | "deny" | "pc" | "reply"; reply?: number; }   // reply: the number of a ready answer (with decision "reply")   // reqId ^[0-9a-f]{16}$
/** M17 (DATA_MODEL.md 10.4): open the program a Windows notification came from. Only the id of the notification travels. */
export interface NotifOpenMsg { t: "notif_open"; notifId: string; }
/** M20 (T20.4): the next piece of the new APK; `offset` is a multiple of 3000. Only inside the encrypted channel. */
export interface ApkGetMsg { t: "apk_get"; offset: number; }
// M21 (DATA_MODEL.md 12.6): voice from the phone, only inside the v2 channel. `seq` counts from 0 and grows by exactly 1.
export interface VoiceStartMsg { t: "voice_start"; id: string; }                                   // id: 16 lower-case hex characters
export interface VoiceChunkMsg { t: "voice_chunk"; id: string; seq: number; data: string; }         // base64 PCM16 16 kHz mono, <= 3200 bytes (100 ms), <= 15 per second
export interface VoiceEndMsg { t: "voice_end"; id: string; }
export interface VoiceCancelMsg { t: "voice_cancel"; id: string; }
export interface VoiceConfirmMsg { t: "voice_confirm"; id: string; yes: boolean; }
/** The `voice` section: the state of the current voice request; null before the first one. */
export interface VoiceInfo {
  state: "idle" | "listening" | "loading" | "recognizing" | "thinking" | "confirm" | "done" | "error";
  id?: string; source?: "pc" | "phone"; heard?: string; reply?: string; confirm?: { text: string; key?: string; args?: Record<string, string> };
  /** i18n key (`voice.reply.*`) of a fixed agent phrase; `reply` is the free text or the fallback. */
  replyKey?: string; replyArgs?: Record<string, string>;
}
export type ClientMsg = Hello2Msg | AuthMsg | PairMsg | HelloMsg | PressMsg | NotifDismissMsg | PingMsg | PermAnswerMsg | NotifOpenMsg | ApkGetMsg
  | VoiceStartMsg | VoiceChunkMsg | VoiceEndMsg | VoiceCancelMsg | VoiceConfirmMsg;

// ===== Server -> Client (WS) =====
/** M18: answer to `hello2`; `sn` is 16 random bytes, base64url. */
export interface ChallengeMsg { t: "challenge"; sn: string; agent: { name: string; version: string }; }
/** M18: the proof of the agent, HMAC-SHA256 in base64url. */
export interface AuthOkMsg { t: "auth_ok"; mac: string; }
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
/** M20 (T20.4): a piece of the new APK, `data` is base64 of at most 3000 bytes; `total` and `sha256` (hex) describe the whole file. */
export interface ApkChunkMsg { t: "apk_chunk"; offset: number; data: string; total: number; sha256: string; }
export type ServerMsg = ChallengeMsg | AuthOkMsg | PairedMsg | WelcomeMsg | PatchMsg | PressResultMsg | PongMsg | ApkChunkMsg | ErrorMsg;

// ===== State (sections) =====
export type SectionName = "layout" | "sys" | "statuses" | "notifications" | "vars" | "focus" | "media" | "icons" | "weather" | "obs" | "ai" | "twitch" | "youtube" | "strip" | "battery" | "mail" | "update" | "voice"; // weather, obs: M15; ai: M16; strip, battery, mail: M17; update: M20; voice: M21
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
  youtube?: YoutubeInfo | null;    // null = YouTube is off or nobody has signed in
  strip?: StripInfo | null;        // M17 (DATA_MODEL.md 10.5); null = no app strip
  battery?: BatteryInfo | null;    // M17 (10.6); null = the collector is not needed or has no data
  mail?: MailInfo | null;          // M17 (10.7); null = no mailboxes
  update?: UpdateInfo2 | null;     // M20 (T20.4); null = no new app for the phone
  voice?: VoiceInfo | null;        // M21
}

/** A newer app, downloaded and verified by the agent: the phone fetches it with `apk_get`. */
export interface UpdateInfo2 { app?: { version: string; size: number; sha256: string } }

/** The keys of the built-in template of the app in front (DATA_MODEL.md 10.5), at most 16. */
export interface StripInfo { app: string; template: string; name: string; keys: { actionId: Id; label: string; icon?: string }[]; icon?: string; } // icon: "img:<hash16>" of the installed program, absent = lucide:app-window
export interface BatteryInfo { devices: { name: string; pct: number; kind: "pc" | "bt"; charging?: boolean }[]; updatedAt: number; } // at most 8
/** `openActionId` (an id of an existing action, never a command) is the key a tap on the mailbox presses; absent = a tap does nothing. */
export interface MailInfo { accounts: { id: Id; name: string; unread: number | null; error?: "auth" | "network"; openActionId?: Id }[]; updatedAt: number; } // at most 3

/** The live numbers of the signed-in YouTube channel. No token and no secret ever appear here. */
export interface YoutubeInfo {
  connected: boolean;              // false = the last look at the channel failed (the numbers are the last good ones)
  channel: string;                 // the channel title
  live: boolean;
  viewers: number;                 // 0 when not live
  subscribers?: number;            // absent when the channel hides its count
  title?: string;                  // <= 140, while live
  startedAt?: number;              // ts the stream began
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
  replies?: string[];              // ready answers of "Deny and say", at most 4
  always?: { kind: "command" | "project" | "host" | "tool"; value: string };   // what "Always allow" would remember; absent = nothing safe to remember
  danger?: boolean;                // M18: a dangerous command; `Allow` is a 1 s hold and `Always allow` is not offered
}
/** One answered question (kept in the agent's memory only). `auto` = a rule of "Always allow" answered it. */
export interface PermLog {
  ts: number;
  provider: "claude" | "codex";
  tool: string;
  summary: string;
  project: string;
  decision: "allow" | "deny" | "pc" | "auto";
}
export interface AiInfo {
  claude?: AiProvider | null;      // null = no data (not installed, or no session yet)
  codex?: AiProvider | null;       // null = off, or Codex not found
  sessions: AiSession[];           // Claude Code sessions seen in the last 10 min, newest first, <= 6
  pending: PermRequest[];          // waiting for an answer, oldest first, <= 5
  history: PermLog[];              // the last answered questions, newest first, <= 30
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
  source?: string;               // <= 40; M17: "windows" = a notification of Windows (DATA_MODEL.md 10.4)
  app?: string;                  // M17: the display name of the Windows app, <= 80
  openable?: boolean;            // M17: a tap sends `notif_open`
  hidden?: boolean;              // M18: the text stayed on the PC; `title` is the program, the phone shows "New notification"
}

// ===== HTTP =====
export interface HealthRes { name: "decka"; version: string; protocol: number; }
export interface NotifyReq { title: string; body?: string; level?: NotifLevel; source?: string; }
export interface NotifyRes { id: string; }
export interface StatusReq { id: Id; state: StatusState; label?: string; text?: string; group?: string; ttlSec?: number; }
export interface OkRes { ok: true; }
export interface VarReq { value: VarValue; }
export interface HttpError { error: { code: ErrorCode; message: string } }
