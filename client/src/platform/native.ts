import { Capacitor } from "@capacitor/core";
import { KeepAwake } from "@capacitor-community/keep-awake";
import type { ClientInfo } from "@decka/shared";

/** True inside the Capacitor app. */
export function isNative(): boolean {
  return Capacitor.isNativePlatform();
}

export function clientInfo(): ClientInfo {
  if (isNative()) return { name: "Android", app: "capacitor", version: import.meta.env.VITE_APP_VERSION };
  return { name: "Browser", app: "web", version: import.meta.env.VITE_APP_VERSION };
}

interface WakeLockSentinelLike {
  release(): Promise<void>;
}

let sentinel: WakeLockSentinelLike | null = null;

async function requestWebWakeLock(): Promise<void> {
  try {
    const wakeLock = (navigator as unknown as { wakeLock?: { request(type: "screen"): Promise<WakeLockSentinelLike> } }).wakeLock;
    if (!wakeLock) return; // not available in an insecure context: do nothing
    sentinel = await wakeLock.request("screen");
  } catch {
    sentinel = null;
  }
}

/** Keeps the screen on: the plugin in the app, the Wake Lock API (re-requested when the tab returns) in a browser. */
export function keepAwake(): void {
  if (isNative()) {
    KeepAwake.keepAwake().catch(() => undefined);
    return;
  }
  void requestWebWakeLock();
  document.addEventListener("visibilitychange", () => {
    if (document.visibilityState === "visible") void requestWebWakeLock();
  });
}
