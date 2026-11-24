import { t, type WeatherItem } from "@decka/shared";
import { formatWeekday } from "./format";
import { useDeckEnv } from "../deckEnv";
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
 * The weather widget (DATA_MODEL.md 6.3, DESIGN.md 4.16):
 * Density is driven by `@container tile`: S shows icon + temp; M adds place & min/max; L adds 3-day daily row.
 * Hourly forecast is skipped because WeatherInfo does not include hourly data.
 */
export default function Weather({ item: _item }: { item: WeatherItem }) {
  const { weather } = useDeckEnv();
  const shownTemp = useAnimatedNumber(weather?.current.temp ?? 0, 300);
  if (!weather) {
    return <WidgetEmpty icon="lucide:sun" text={t("widget.no_data")} />;
  }
  const { current, daily, place } = weather;
  const today = daily[0];
  const icon = weatherIcon(current.code, current.isDay);
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
      <div className="weather-days w-l">
        {daily.slice(0, 3).map((d) => (
          <div key={d.date} className="weather-day">
            <span className="weather-dow">{weekday(d.date)}</span>
            <WeatherGlyph name={weatherIcon(d.code, true)} size={20} />
            <span className="weather-day-temp">{deg(d.max)} / {deg(d.min)}</span>
          </div>
        ))}
      </div>
    </div>
  );
}
