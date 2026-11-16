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
  };
}
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
