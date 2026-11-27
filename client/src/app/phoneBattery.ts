export interface BatteryManagerLike {
  level: number;
  charging: boolean;
  addEventListener(type: "levelchange" | "chargingchange", listener: () => void): void;
  removeEventListener(type: "levelchange" | "chargingchange", listener: () => void): void;
}

export interface NavigatorWithBattery {
  getBattery?: () => Promise<BatteryManagerLike>;
}

export const PHONE_BATTERY_INTERVAL_MS = 60_000;

/**
 * T30.2b: Reads `navigator.getBattery()` without external dependencies and sends `phone_status`
 * on change and every 60 seconds. If the API is unavailable, does nothing.
 */
export function startPhoneBatteryMonitor(
  send: (battery: number, charging: boolean) => void,
  nav: NavigatorWithBattery | undefined = typeof navigator !== "undefined" ? (navigator as NavigatorWithBattery) : undefined,
): () => void {
  if (!nav || typeof nav.getBattery !== "function") return () => {};

  let stopped = false;
  let timer: ReturnType<typeof setInterval> | null = null;
  let manager: BatteryManagerLike | null = null;
  let lastPct = -1;
  let lastCharging: boolean | null = null;

  const readSnapshot = (mgr: BatteryManagerLike) => ({
    battery: Math.max(0, Math.min(100, Math.round(mgr.level * 100))),
    charging: Boolean(mgr.charging),
  });

  const emitIfChanged = () => {
    if (stopped || !manager) return;
    const snap = readSnapshot(manager);
    if (snap.battery !== lastPct || snap.charging !== lastCharging) {
      lastPct = snap.battery;
      lastCharging = snap.charging;
      send(snap.battery, snap.charging);
    }
  };

  const emitPeriodic = () => {
    if (stopped || !manager) return;
    const snap = readSnapshot(manager);
    lastPct = snap.battery;
    lastCharging = snap.charging;
    send(snap.battery, snap.charging);
  };

  nav
    .getBattery()
    .then((mgr) => {
      if (stopped || !mgr) return;
      manager = mgr;
      emitPeriodic();
      mgr.addEventListener("levelchange", emitIfChanged);
      mgr.addEventListener("chargingchange", emitIfChanged);
      timer = setInterval(emitPeriodic, PHONE_BATTERY_INTERVAL_MS);
    })
    .catch(() => {});

  return () => {
    stopped = true;
    if (timer !== null) clearInterval(timer);
    if (manager) {
      manager.removeEventListener("levelchange", emitIfChanged);
      manager.removeEventListener("chargingchange", emitIfChanged);
    }
  };
}
