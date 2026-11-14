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
export type ClientMsg = PairMsg | HelloMsg | PressMsg | NotifDismissMsg | PingMsg;

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
export type SectionName = "layout" | "sys" | "statuses" | "notifications" | "vars" | "focus" | "media" | "icons";
export interface FullState {
  layout: ClientLayout;
  sys: SysStats | null;            // null = collector disabled
  statuses: StatusItem[];          // sorted by (group, id)
  notifications: Notification[];   // newest first, at most 20
  vars: Record<VarName, VarValue>;
  focus?: FocusInfo | null;        // M11; absent from agents older than M11; null = collector disabled
  media?: MediaInfo | null;        // M11; null = nothing playing or collector disabled
  icons?: Record<string, string>;  // M12: "<hash16>" -> "data:image/png;base64,..."; only the images the layout refers to; absent = {}
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
