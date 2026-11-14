import type { ClientInfo } from "@decka/shared";

/** True inside the Capacitor app. Always false until T7.2 adds Capacitor. */
export function isNative(): boolean {
  return false;
}

export function clientInfo(): ClientInfo {
  return { name: "Browser", app: "web", version: import.meta.env.VITE_APP_VERSION };
}
