import { t, type WeatherItem } from "@decka/shared";
import { formatWeekday } from "./format";
import { useDeckEnv } from "../deckEnv";
import type { WeatherInfoWithHourly } from "./m24Types";
import { weatherIcon, type WeatherIconName } from "./weatherIcon";
import { WEATHER_SVG } from "./weatherIcons";
import WidgetEmpty from "./WidgetEmpty";
import { useAnimatedNumber } from "./useAnimatedNumber";

/** A weather icon drawn as a mask in the colour of the text (it follows the theme). */
function WeatherGlyph({ name, size }: { name: WeatherIconName; size: number }) {
  const mask = `url("data:image/svg+xml;utf8,${encodeURIComponent(WEATHER_SVG[name])}")`;
  return (
    <span
      aria-hidden="true"
      className="weather-glyph"
      style={{ width: size, height: size, backgroundColor: "currentColor", WebkitMaskImage: mask, maskImage: mask, WebkitMaskSize: "contain", maskSize: "contain", WebkitMaskRepeat: "no-repeat", maskRepeat: "no-repeat", WebkitMaskPosition: "center", maskPosition: "center" }}
    />
  );
}

/** `2026-11-17` -> a short weekday in the language of the interface (`Tue`, `вт`). */
function weekday(date: string): string {
  const d = new Date(`${date}T12:00:00`);
  return Number.isNaN(d.getTime()) ? "" : formatWeekday(d);
}

const deg = (n: number): string => `${n}°`;

/**
 * The weather widget (DATA_MODEL.md 6.3, DESIGN.md 4.16, 4.17):
 * Density is driven by `@container tile`: S shows icon + temp; M adds place & min/max;
 * L adds the 6-column hourly row when `hourly` is present (falling back to 3-day daily otherwise).
 */
export default function Weather({ item: _item }: { item: WeatherItem }) {
  const { weather } = useDeckEnv();
  const shownTemp = useAnimatedNumber(weather?.current.temp ?? 0, 300);
  if (!weather) {
    return <WidgetEmpty icon="lucide:sun" text={t("widget.no_data")} />;
  }
  const w = weather as WeatherInfoWithHourly;
  const { current, daily, place, hourly } = w;
  const today = daily[0];
  const icon = weatherIcon(current.code, current.isDay);
  const hasHourly = Array.isArray(hourly) && hourly.length > 0;
  return (
    <div className="weather">
      <div className="weather-now">
        <WeatherGlyph name={icon} size={36} />
        <span className="weather-temp widget-figure">{deg(shownTemp)}</span>
        <span className="weather-side w-m">
          {today && <span className="weather-range">{deg(today.min)} / {deg(today.max)}</span>}
          <span className="weather-place">{place}</span>
        </span>
      </div>
      {hasHourly ? (
        <div className="weather-hourly w-l">
          {hourly.slice(0, 6).map((h, i) => (
            <div key={`${h.time}-${i}`} className="weather-hour">
              <span className="weather-hour-time">{h.time}</span>
              <WeatherGlyph name={weatherIcon(h.code, h.isDay)} size={20} />
              <span className="weather-hour-temp">{deg(h.temp)}</span>
            </div>
          ))}
        </div>
      ) : (
        <div className="weather-days w-l">
          {daily.slice(0, 3).map((d) => (
            <div key={d.date} className="weather-day">
              <span className="weather-dow">{weekday(d.date)}</span>
              <WeatherGlyph name={weatherIcon(d.code, true)} size={20} />
              <span className="weather-day-temp">{deg(d.max)} / {deg(d.min)}</span>
            </div>
          ))}
        </div>
      )}
    </div>
  );
}
