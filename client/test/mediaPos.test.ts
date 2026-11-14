import { describe, expect, test } from "vitest";
import type { MediaInfo } from "@decka/shared";
import { currentPosition, formatTime } from "../src/deck/widgets/mediaPos";

const media = (over: Partial<MediaInfo> = {}): MediaInfo => ({
  source: "Spotify.exe",
  title: "Midnight City",
  playing: true,
  positionSec: 42,
  durationSec: 244,
  positionAt: 1_000_000,
  ...over,
});

describe("currentPosition", () => {
  test("a playing track moves with the clock", () => {
    expect(currentPosition(media(), 1_000_000)).toBe(42);
    expect(currentPosition(media(), 1_010_500)).toBe(52);
  });

  test("a paused track stays at positionSec", () => {
    expect(currentPosition(media({ playing: false }), 1_500_000)).toBe(42);
  });

  test("no positionSec is unknown", () => {
    const m = media();
    delete m.positionSec;
    expect(currentPosition(m, 1_010_000)).toBeNull();
  });

  test("never beyond durationSec", () => {
    expect(currentPosition(media(), 1_900_000)).toBe(244);
  });

  test("without a duration it keeps counting", () => {
    const m = media();
    delete m.durationSec;
    expect(currentPosition(m, 1_100_000)).toBe(142);
  });

  test("a clock that is behind the agent does not move the position back", () => {
    expect(currentPosition(media(), 990_000)).toBe(42);
  });
});

describe("formatTime", () => {
  test("m:ss", () => {
    expect(formatTime(0)).toBe("0:00");
    expect(formatTime(5)).toBe("0:05");
    expect(formatTime(244)).toBe("4:04");
    expect(formatTime(3599)).toBe("59:59");
  });

  test("h:mm:ss from an hour", () => {
    expect(formatTime(3600)).toBe("1:00:00");
    expect(formatTime(3725)).toBe("1:02:05");
  });

  test("negative and fractional input", () => {
    expect(formatTime(-3)).toBe("0:00");
    expect(formatTime(61.9)).toBe("1:01");
  });
});
