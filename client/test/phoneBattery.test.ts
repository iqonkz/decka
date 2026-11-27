import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { PHONE_BATTERY_INTERVAL_MS, startPhoneBatteryMonitor, type BatteryManagerLike } from "../src/app/phoneBattery";

describe("T30.2b startPhoneBatteryMonitor", () => {
  beforeEach(() => {
    vi.useFakeTimers();
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it("does nothing when navigator.getBattery is absent", () => {
    const send = vi.fn();
    const stop = startPhoneBatteryMonitor(send, {});
    expect(send).not.toHaveBeenCalled();
    stop();
  });

  it("sends phone_status initially, on level/charging change, and every 60 seconds", async () => {
    const listeners: Record<string, Set<() => void>> = {
      levelchange: new Set(),
      chargingchange: new Set(),
    };
    const mgr: BatteryManagerLike = {
      level: 0.84,
      charging: false,
      addEventListener: (type, fn) => listeners[type]?.add(fn),
      removeEventListener: (type, fn) => listeners[type]?.delete(fn),
    };
    const send = vi.fn();
    const stop = startPhoneBatteryMonitor(send, { getBattery: () => Promise.resolve(mgr) });

    await Promise.resolve();
    await Promise.resolve();
    expect(send).toHaveBeenCalledTimes(1);
    expect(send).toHaveBeenLastCalledWith(84, false);

    mgr.level = 0.83;
    mgr.charging = true;
    for (const fn of listeners.levelchange!) fn();
    expect(send).toHaveBeenCalledTimes(2);
    expect(send).toHaveBeenLastCalledWith(83, true);

    vi.advanceTimersByTime(PHONE_BATTERY_INTERVAL_MS);
    expect(send).toHaveBeenCalledTimes(3);
    expect(send).toHaveBeenLastCalledWith(83, true);

    stop();
  });
});
