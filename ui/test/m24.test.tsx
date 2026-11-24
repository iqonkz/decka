import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, test } from "vitest";
import type { LayoutItem, WeatherInfo } from "@decka/shared";
import { DeckEnvContext, EMPTY_DATA, type DeckData, type DeckEnv } from "../src/deckEnv";
import { DeckMetricsContext } from "../src/metricsContext";
import Tile from "../src/Tile";
import type { WeatherInfoWithHourly } from "../src/widgets/m24Types";

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
