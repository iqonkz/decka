import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, test } from "vitest";
import type { LayoutItem, WeatherInfo } from "@decka/shared";
import { DeckEnvContext, EMPTY_DATA, type DeckData, type DeckEnv } from "../src/deckEnv";
import { DeckMetricsContext } from "../src/metricsContext";
import Tile from "../src/Tile";
import { effectiveMediaPositionMs, formatOffsetSeconds, selectActiveLyricIndex } from "../src/widgets/lyricsLogic";
import { formatRepeatLabel } from "../src/widgets/repeatLabel";
import type { LyricsInfo, RemindersSectionWithRepeat, WeatherInfoWithHourly } from "../src/widgets/m24Types";

const NOOP = () => {};

function renderTile(item: LayoutItem, data: Partial<DeckData> = {}) {
  const env: DeckEnv = {
    ...EMPTY_DATA,
    online: true,
    ...data,
    onPress: () => Promise.resolve(),
    onPressError: NOOP,
    onNavigate: NOOP,
    onDismiss: NOOP,
  };
  return renderToStaticMarkup(
    <DeckEnvContext.Provider value={env}>
      <DeckMetricsContext.Provider value={{ side: 100, gap: 4 }}>
        <Tile item={item} />
      </DeckMetricsContext.Provider>
    </DeckEnvContext.Provider>,
  );
}

const baseWeather: WeatherInfo = {
  place: "Almaty",
  units: "metric",
  updatedAt: 1_700_000_000_000,
  current: { temp: 7, code: 2, isDay: true, wind: 10 },
  daily: [
    { date: "2026-11-25", min: 2, max: 9, code: 2 },
    { date: "2026-11-26", min: 1, max: 6, code: 61 },
    { date: "2026-11-27", min: -1, max: 4, code: 71 },
  ],
};

describe("weather L hourly forecast (DESIGN 4.17)", () => {
  test("hourly row is rendered with 6 columns only when hourly data exists", () => {
    const item = { id: "w1", kind: "weather", x: 0, y: 0, w: 3, h: 2 } as LayoutItem;
    const withoutHourly = renderTile(item, { weather: baseWeather });
    expect(withoutHourly).not.toContain('class="weather-hourly');
    expect(withoutHourly).toContain('class="weather-days');

    const withHourly: WeatherInfoWithHourly = {
      ...baseWeather,
      hourly: [
        { time: "13:00", temp: 8, code: 1, isDay: true },
        { time: "14:00", temp: 9, code: 2, isDay: true },
        { time: "15:00", temp: 8, code: 2, isDay: true },
        { time: "16:00", temp: 6, code: 3, isDay: true },
        { time: "17:00", temp: 4, code: 61, isDay: false },
        { time: "18:00", temp: 3, code: 61, isDay: false },
        { time: "19:00", temp: 2, code: 3, isDay: false },
      ],
    };
    const html = renderTile(item, { weather: withHourly });
    expect(html).toContain("weather-hourly");
    expect((html.match(/class="weather-hour"/g) ?? []).length).toBe(6);
    expect(html).toContain("13:00");
    expect(html).toContain("18:00");
    expect(html).not.toContain("19:00");
  });
});

describe("media L lyrics & sync (DESIGN 4.17, DATA_MODEL 14)", () => {
  const sampleLines = [
    { t: 1000, text: "First line" },
    { t: 5000, text: "Second line" },
    { t: 9000, text: "Third line" },
  ];

  test("effectiveMediaPositionMs accounts for elapsed playback + offsetMs, and freezes when paused", () => {
    const playingMedia = { playing: true, positionSec: 4, positionAt: 10_000 };
    expect(effectiveMediaPositionMs(playingMedia, 11_500, 0)).toBe(5500);
    expect(effectiveMediaPositionMs(playingMedia, 11_500, -1000)).toBe(4500);
    expect(effectiveMediaPositionMs(playingMedia, 11_500, 3500)).toBe(9000);

    const pausedMedia = { playing: false, positionSec: 4, positionAt: 10_000 };
    expect(effectiveMediaPositionMs(pausedMedia, 19_000, 0)).toBe(4000);
    expect(effectiveMediaPositionMs(pausedMedia, 19_000, 1500)).toBe(5500);
    expect(effectiveMediaPositionMs(pausedMedia, 19_000, -500)).toBe(3500);
  });

  test("selectActiveLyricIndex picks the last line with t <= effectivePositionMs", () => {
    expect(selectActiveLyricIndex(sampleLines, 999)).toBe(-1);
    expect(selectActiveLyricIndex(sampleLines, 1000)).toBe(0);
    expect(selectActiveLyricIndex(sampleLines, 4999)).toBe(0);
    expect(selectActiveLyricIndex(sampleLines, 5000)).toBe(1);
    expect(selectActiveLyricIndex(sampleLines, 9000)).toBe(2);
    expect(selectActiveLyricIndex([{ text: "No timestamp" }], 5000)).toBe(-1);
  });

  test("formatOffsetSeconds formats positive, zero, and negative offsets", () => {
    expect(formatOffsetSeconds(1000)).toBe("+1.0");
    expect(formatOffsetSeconds(0)).toBe("0.0");
    expect(formatOffsetSeconds(-500)).toBe("−0.5");
  });

  test("Media widget renders synced or plain lyrics only in L (w >= 4, h >= 2)", () => {
    const media = {
      source: "spotify",
      title: "Midnight City",
      artist: "M83",
      playing: false,
      positionSec: 5,
      durationSec: 244,
      positionAt: 10_000,
    };
    const syncedLyrics: LyricsInfo = { trackKey: "0123456789abcdef", synced: true, lines: sampleLines, offsetMs: 0 };
    const plainLyrics: LyricsInfo = { trackKey: "0123456789abcdef", synced: false, lines: sampleLines.map((l) => ({ text: l.text })), offsetMs: 0 };

    const small = { id: "m1", kind: "media", x: 0, y: 0, w: 3, h: 1 } as LayoutItem;
    expect(renderTile(small, { media, lyrics: syncedLyrics })).not.toContain("media-lyrics");

    const medium = { id: "m2", kind: "media", x: 0, y: 0, w: 2, h: 2 } as LayoutItem;
    expect(renderTile(medium, { media, lyrics: syncedLyrics })).not.toContain("media-lyrics");

    const large = { id: "m3", kind: "media", x: 0, y: 0, w: 4, h: 2 } as LayoutItem;
    const syncedHtml = renderTile(large, { media, lyrics: syncedLyrics });
    expect(syncedHtml).toContain("media-lyrics");
    expect(syncedHtml).toContain("media-lyric-line active");
    expect(syncedHtml).not.toContain("media-lyrics-plain");

    const plainHtml = renderTile(large, { media, lyrics: plainLyrics });
    expect(plainHtml).toContain("media-lyrics-plain");
    expect(plainHtml).not.toContain("media-lyric-line active");
  });
});

describe("recurring reminders repeat label & widget (DATA_MODEL 13.4)", () => {
  test("formatRepeatLabel formats daily, weekdays, weekly, and omits non-repeating", () => {
    expect(formatRepeatLabel(undefined, "2026-11-23T09:00", "en")).toBeNull();
    expect(formatRepeatLabel("daily", "2026-11-23T09:00", "en")).toBe("Every day");
    expect(formatRepeatLabel("weekdays", "2026-11-23T09:00", "en")).toBe("Weekdays");
    expect(formatRepeatLabel("weekly", "2026-11-23T09:00", "en")).toBe("Every Mon");
    expect(formatRepeatLabel("weekly", "2026-11-23T09:00", "ru")).toMatch(/Mon|пн/i);
  });

  test("Reminders phone widget renders repeat icon + label for repeating reminders", () => {
    const reminders: RemindersSectionWithRepeat = {
      items: [
        { id: "0000000000000001", text: "Standup", at: "2026-11-25T10:00", repeat: "weekdays" },
        { id: "0000000000000002", text: "Vitamins", at: "2026-11-25T08:00", repeat: "daily" },
        { id: "0000000000000003", text: "Call mom", at: "2026-11-25T18:00" },
      ],
    };
    const item = { id: "r1", kind: "reminders", x: 0, y: 0, w: 3, h: 2 } as LayoutItem;
    const html = renderTile(item, { reminders: reminders as never });
    expect(html).toContain('class="rem-repeat"');
    expect(html).toContain("Weekdays");
    expect(html).toContain("Every day");
    expect((html.match(/class="rem-repeat"/g) ?? []).length).toBe(2);
  });
});
