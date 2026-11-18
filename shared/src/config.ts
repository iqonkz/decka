import type { Id } from "./protocol";

export interface ConfigFile {
  version: 1;
  agentName: string;                 // 1..40; default = computer name (env COMPUTERNAME)
  port: number;                      // 1024..65535; default 9247
  bind: "0.0.0.0" | "127.0.0.1";     // default "0.0.0.0"; "127.0.0.1" = USB-only mode
  apiToken: string;                  // "dk_" + 43 base64url chars; generated on first start
  autostart: boolean;                // default false
  updateCheck?: boolean;             // M14: ask GitHub once a day for a newer release (ARCHITECTURE.md 8a); absent = true
  usb: { enabled: boolean; adbPath: string };   // default { false, "adb" }
  collectors: {
    sys: { enabled: boolean; intervalSec: number };   // default { true, 2 }; 1..60
    checks: HttpCheck[];                              // default []; max 20
    focus?: { enabled: boolean };                     // M11; absent = { true }
    media?: { enabled: boolean };                     // M11; absent = { true }
    audio?: { enabled: boolean };                     // M15 (DATA_MODEL.md 6.6); absent = { true }
  };
  weather?: WeatherConfig;           // M15 (DATA_MODEL.md 6.3); absent = off
  obs?: ObsConfig;                   // M15 (DATA_MODEL.md 6.4); absent = off
  ai?: AiConfig;                     // M16 (DATA_MODEL.md 6.8); absent = off
  twitch?: TwitchConfig;             // absent = off
  youtube?: YoutubeConfig;           // absent = off
}
/** The client id is the id of the owner's own Google OAuth client ("TVs and Limited Input devices"). The client secret
 * (`set_youtube_secret`) and the sign-in (`channel`, the tokens) are written by Decka only; `get_config` leaves the secret and the
 * tokens out and says `secretSet` / `tokensSet`. */
export interface YoutubeConfig { enabled: boolean; clientId: string; secret?: string; channel?: string; tokens?: string; secretSet?: boolean; tokensSet?: boolean; }
/** The client id is the id of the owner's own Twitch application (a public client: there is no secret). The sign-in
 * (`login`, `userId`, the tokens) is written by the sign-in only; `get_config` leaves the tokens out and says `tokensSet`. */
export interface TwitchConfig { enabled: boolean; clientId: string; login?: string; userId?: string; tokens?: string; tokensSet?: boolean; }
/** `hookToken` is made by `cc_install` and never reaches the window. */
export interface AiConfig {
  claude: { enabled: boolean; approvals: boolean; timeoutSec: 60 | 120 | 300 };
  codex: { enabled: boolean; approvals?: boolean };   // approvals: phone approvals for Codex, absent = on
  hookToken?: string;
  rules?: AiRule[];                                   // what the phone's "Always allow" remembered; removed in the window
}
/** `kind`: `command` (value = the first words), `project` (value = the folder), `host` (value = the site), `tool` (value empty). */
export interface AiRule { id: string; provider: "claude" | "codex"; tool: string; kind: "command" | "project" | "host" | "tool"; value: string; createdAt: number; }
export interface WeatherConfig { enabled: boolean; place: string; lat: number; lon: number; units: "metric" | "imperial"; }
/** `password` is only ever "dpapi:<base64>" on disk and is written only by `set_obs_password`; `get_config` leaves it out and says `passwordSet`. */
export interface ObsConfig { enabled: boolean; port: number; password?: string; passwordSet?: boolean; }
export interface HttpCheck {
  id: Id; label: string;             // label 1..60
  url: string;                       // http/https, <= 2000
  intervalSec: number;               // 10..3600
  timeoutSec: number;                // 1..30
  expectStatus?: number;             // default: 200..399 = ok
  group?: string;                    // <= 40
}

export interface DeviceInfo { id: Id; name: string; createdAt: number; lastSeenAt: number; connected: boolean; appVersion?: string; } // appVersion: client.version of the last pair/hello (M14)
// devices.json on disk:
// { "version": 1, "devices": [ { "id", "name", "tokenHash", "createdAt", "lastSeenAt" } ] }
// tokenHash = hex(SHA-256(token)); the token itself is NEVER written to disk.
