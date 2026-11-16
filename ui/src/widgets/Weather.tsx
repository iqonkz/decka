import { locale, type WeatherItem } from "@decka/shared";
import { useDeckEnv } from "../deckEnv";
import { weatherIcon, type WeatherIconName } from "./weatherIcon";
import { WEATHER_SVG } from "./weatherIcons";

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
  return Number.isNaN(d.getTime()) ? "" : new Intl.DateTimeFormat(locale(), { weekday: "short" }).format(d);
}

const deg = (n: number): string => `${n}°`;

/** The weather widget (DATA_MODEL.md 6.3): 1x1 icon and temperature; 2x1 adds min / max and the place; 3x2 and 4x2 add three days. */
export default function Weather({ item }: { item: WeatherItem }) {
  const { weather } = useDeckEnv();
  if (!weather) {
    return (
      <div className="weather weather-empty">
        <WeatherGlyph name="cloud" size={Math.round(0.9 * 28)} />
        <span className="weather-temp">—</span>
      </div>
    );
  }
  const { current, daily, place } = weather;
  const today = daily[0];
  const icon = weatherIcon(current.code, current.isDay);
  const big = item.h >= 2;
  return (
    <div className={big ? "weather big" : "weather"}>
      <div className="weather-now">
        <WeatherGlyph name={icon} size={big ? 44 : 32} />
        <span className="weather-temp">{deg(current.temp)}</span>
        {item.w >= 2 && (
          <span className="weather-side">
            {today && <span className="weather-range">{deg(today.min)} / {deg(today.max)}</span>}
            <span className="weather-place">{place}</span>
          </span>
        )}
      </div>
      {big && (
        <div className="weather-days">
          {daily.slice(0, 3).map((d) => (
            <div key={d.date} className="weather-day">
              <span className="weather-dow">{weekday(d.date)}</span>
              <WeatherGlyph name={weatherIcon(d.code, true)} size={26} />
              <span className="weather-range">{deg(d.max)} / {deg(d.min)}</span>
            </div>
          ))}
        </div>
      )}
    </div>
  );
}
