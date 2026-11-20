import { describe, expect, test } from "vitest";
import { VOLUME_STEP, volumeAt, volumeSteps } from "../src/widgets/volumeSteps";
import { formatRemaining, timerInit, timerReducer, type TimerState } from "../src/widgets/timerState";
import { weatherIcon } from "../src/widgets/weatherIcon";
import { HISTORY_LENGTH, sparklinePath } from "../src/widgets/sparkline";
import { obsElapsed } from "../src/widgets/obsTime";

describe("volumeSteps", () => {
  test("steps of 2 %, in the direction of the target", () => {
    expect(volumeSteps(40, 50)).toEqual({ action: "up", count: 5 });
    expect(volumeSteps(50, 40)).toEqual({ action: "down", count: 5 });
    expect(volumeSteps(0, 100)).toEqual({ action: "up", count: 50 });
    expect(VOLUME_STEP).toBe(2);
  });

  test("no move is no press; a difference below one step stays put", () => {
    expect(volumeSteps(30, 30).count).toBe(0);
    expect(volumeSteps(30, 31).count).toBe(0);
    expect(volumeSteps(30, 33).count).toBe(1);
    expect(volumeSteps(33, 30)).toEqual({ action: "down", count: 1 });
  });

  test("the press loop is finite and never changes direction", () => {
    for (const [from, to] of [[37, 40], [41, 40], [0, 100], [100, 0], [1, 2]] as const) {
      let shown: number = from;
      const dirs = new Set<string>();
      let n = 0;
      for (;; n++) {
        const { action, count } = volumeSteps(shown, to);
        if (count === 0) break;
        dirs.add(action);
        shown += action === "up" ? VOLUME_STEP : -VOLUME_STEP;
        expect(n).toBeLessThan(100);
      }
      expect(dirs.size).toBeLessThanOrEqual(1);
    }
  });

  test("the value under the finger is clamped to 0..100; a vertical track grows upwards", () => {
    expect(volumeAt(150, 100, 200, false)).toBe(25);
    expect(volumeAt(50, 100, 200, false)).toBe(0);
    expect(volumeAt(500, 100, 200, false)).toBe(100);
    expect(volumeAt(150, 100, 200, true)).toBe(75); // near the top of the track = loud
    expect(volumeAt(300, 100, 200, true)).toBe(0);
    expect(volumeAt(10, 0, 0, false)).toBe(0);
  });
});

describe("timerReducer", () => {
  const start = (s: TimerState, now: number) => timerReducer(s, { type: "toggle", now });

  test("a new timer waits with its whole length", () => {
    expect(timerInit(25)).toEqual({ minutes: 25, remainingMs: 1_500_000, running: false, endsAt: null, done: false });
  });

  test("tap starts, ticks count down, tap pauses and tap resumes where it stopped", () => {
    let s = start(timerInit(1), 1000);
    expect(s).toMatchObject({ running: true, endsAt: 61_000 });
    s = timerReducer(s, { type: "tick", now: 21_000 });
    expect(s.remainingMs).toBe(40_000);
    s = start(s, 26_000); // pause at 35 s left
    expect(s).toMatchObject({ running: false, endsAt: null, remainingMs: 35_000 });
    s = timerReducer(s, { type: "tick", now: 99_000 }); // a paused timer does not move
    expect(s.remainingMs).toBe(35_000);
    s = start(s, 100_000);
    expect(s).toMatchObject({ running: true, endsAt: 135_000 });
  });

  test("it ends at zero, once", () => {
    let s = start(timerInit(1), 0);
    s = timerReducer(s, { type: "tick", now: 59_999 });
    expect(s).toMatchObject({ running: true, done: false });
    s = timerReducer(s, { type: "tick", now: 60_000 });
    expect(s).toMatchObject({ running: false, done: true, remainingMs: 0, endsAt: null });
    expect(timerReducer(s, { type: "tick", now: 70_000 })).toBe(s);
  });

  test("a tap after the end starts the whole length again", () => {
    const ended: TimerState = { minutes: 5, remainingMs: 0, running: false, endsAt: null, done: true };
    expect(start(ended, 10)).toMatchObject({ running: true, done: false, remainingMs: 300_000, endsAt: 300_010 });
  });

  test("the hold resets; a preset picks the length and resets", () => {
    let s = start(timerInit(25), 0);
    s = timerReducer(s, { type: "reset" });
    expect(s).toEqual(timerInit(25));
    s = timerReducer(start(timerInit(25), 0), { type: "preset", minutes: 5 });
    expect(s).toEqual(timerInit(5));
  });

  test("the time reads mm:ss, h:mm:ss from an hour, and a started second counts", () => {
    expect([formatRemaining(1_500_000), formatRemaining(59_001), formatRemaining(0), formatRemaining(3_725_000), formatRemaining(-5)]).toEqual(["25:00", "01:00", "00:00", "1:02:05", "00:00"]);
  });
});

describe("weatherIcon (DESIGN.md 2.3)", () => {
  test.each([
    [0, true, "sun"], [0, false, "moon"], [1, true, "cloud-sun"], [2, false, "cloud-moon"], [3, true, "cloud"], [3, false, "cloud"],
    [45, true, "cloud-fog"], [48, false, "cloud-fog"],
    [51, true, "cloud-rain"], [67, true, "cloud-rain"], [80, true, "cloud-rain"], [82, false, "cloud-rain"],
    [71, true, "cloud-snow"], [77, true, "cloud-snow"], [85, true, "cloud-snow"], [86, false, "cloud-snow"],
    [95, true, "cloud-lightning"], [99, false, "cloud-lightning"],
    [68, true, "cloud"], [70, true, "cloud"], [83, true, "cloud"], [100, true, "cloud"], [-1, true, "cloud"],
  ] as const)("code %i day=%s -> %s", (code, isDay, icon) => expect(weatherIcon(code, isDay)).toBe(icon));
});

describe("sparklinePath", () => {
  test("fewer than two readings draw nothing", () => {
    expect(sparklinePath([], 100, 40)).toEqual({ line: "", area: "" });
    expect(sparklinePath([50], 100, 40)).toEqual({ line: "", area: "" });
  });

  test("a full history spans the width; 0 is the bottom and 100 the top", () => {
    const values = Array.from({ length: HISTORY_LENGTH }, (_, i) => (i === 0 ? 0 : 100));
    const { line, area } = sparklinePath(values, 118, 40);
    expect(line.startsWith("M0 40 L2 0 ")).toBe(true);
    expect(line.endsWith("L118 0")).toBe(true);
    expect(area.endsWith("L118 40 L0 40 Z")).toBe(true);
  });

  test("a short history starts part-way in and ends at the right edge", () => {
    const { line } = sparklinePath([10, 20, 30], 59, 100);
    expect(line).toBe("M57 90 L58 80 L59 70");
  });

  test("only the last 60 are drawn, and out-of-range values are clamped", () => {
    const many = Array.from({ length: 100 }, () => 50);
    expect(sparklinePath(many, 59, 10).line.split("L").length).toBe(HISTORY_LENGTH);
    expect(sparklinePath([-5, 250], 59, 10).line).toBe("M58 10 L59 0");
  });
});

describe("obsElapsed", () => {
  test("HH:MM:SS since the start, whole seconds", () => {
    expect(obsElapsed(1000, 1000)).toBe("00:00:00");
    expect(obsElapsed(0, 59_999)).toBe("00:00:59");
    expect(obsElapsed(0, 754_000)).toBe("00:12:34");
    expect(obsElapsed(0, 3_723_000)).toBe("01:02:03");
    expect(obsElapsed(0, 100 * 3600_000)).toBe("100:00:00");
  });

  test("a start in the future (clock skew) reads zero", () => {
    expect(obsElapsed(5000, 1000)).toBe("00:00:00");
  });
});
