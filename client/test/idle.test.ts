import { describe, expect, it } from "vitest";
import type { DisplaySettings } from "@decka/shared";
import { computeMode, inNight } from "../src/display/idle";

const display = (over: Partial<DisplaySettings> = {}): DisplaySettings => ({
  dimAfterSec: 60,
  dimLevel: 0.3,
  sleepAfterSec: 600,
  pixelShift: true,
  night: { enabled: false, from: "23:00", to: "07:00" },
  ...over,
});
const at = (hh: number, mm: number) => hh * 60 + mm;
const mode = (idleSec: number, d: DisplaySettings, extra: { offlineForSec?: number; nowMinutes?: number } = {}) => {
  const now = 1_000_000_000;
  return computeMode({
    now,
    lastActivity: now - idleSec * 1000,
    display: d,
    offlineSince: extra.offlineForSec === undefined ? null : now - extra.offlineForSec * 1000,
    nowMinutes: extra.nowMinutes ?? at(12, 0),
  });
};

describe("computeMode", () => {
  it("rule 1: under 10 s of idle time is always active", () => {
    expect(mode(9, display({ dimAfterSec: 10, sleepAfterSec: 30 }), { offlineForSec: 100, nowMinutes: at(23, 30) })).toBe("active");
  });

  it("rule 2: a connection lost for 30 s means sleep", () => {
    expect(mode(20, display(), { offlineForSec: 30 })).toBe("sleep");
    expect(mode(20, display(), { offlineForSec: 29 })).toBe("active");
  });

  it("rule 3: night means sleep", () => {
    const night = display({ night: { enabled: true, from: "23:00", to: "07:00" } });
    expect(mode(20, night, { nowMinutes: at(23, 30) })).toBe("sleep");
    expect(mode(20, display(), { nowMinutes: at(23, 30) })).toBe("active"); // night disabled
  });

  it("rule 4: sleepAfterSec", () => {
    expect(mode(600, display())).toBe("sleep");
    expect(mode(599, display())).toBe("dim");
    expect(mode(100000, display({ sleepAfterSec: 0 }))).toBe("dim");
  });

  it("rule 5: dimAfterSec, 0 turns dimming off", () => {
    expect(mode(60, display())).toBe("dim");
    expect(mode(59, display())).toBe("active");
    expect(mode(100000, display({ dimAfterSec: 0, sleepAfterSec: 0 }))).toBe("active");
  });

  it("rule 6: otherwise active", () => {
    expect(mode(30, display())).toBe("active");
  });

  it("activity 5 s ago at night stays active", () => {
    const night = display({ night: { enabled: true, from: "23:00", to: "07:00" } });
    expect(mode(5, night, { nowMinutes: at(2, 0) })).toBe("active");
  });
});

describe("inNight", () => {
  const night = { enabled: true, from: "23:00", to: "07:00" };
  it("crosses midnight", () => {
    expect(inNight(night, at(23, 30))).toBe(true);
    expect(inNight(night, at(6, 59))).toBe(true);
    expect(inNight(night, at(7, 0))).toBe(false);
    expect(inNight(night, at(22, 59))).toBe(false);
    expect(computeModeAtNight(at(7, 0))).not.toBe("sleep");
  });
  it("same-day interval", () => {
    const day = { enabled: true, from: "01:00", to: "05:00" };
    expect(inNight(day, at(1, 0))).toBe(true);
    expect(inNight(day, at(5, 0))).toBe(false);
  });
  it("from === to is never night", () => {
    expect(inNight({ enabled: true, from: "03:00", to: "03:00" }, at(3, 0))).toBe(false);
  });
});

function computeModeAtNight(nowMinutes: number) {
  return mode(20, display({ night: { enabled: true, from: "23:00", to: "07:00" } }), { nowMinutes });
}
