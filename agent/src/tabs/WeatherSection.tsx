import { useState } from "react";
import { t, type ConfigFile, type PlaceResult, type WeatherConfig } from "@decka/shared";
import { call } from "../ipc";
import Banner from "../ui/Banner";
import Button from "../ui/Button";

const label = (p: PlaceResult): string => [...new Set([p.name, p.region, p.country].filter((x): x is string => !!x))].join(", ");

/** `More → Integrations → Weather` (DATA_MODEL.md 6.3): switch, search for a city, units, the Open-Meteo credit. */
export default function WeatherSection({ config, onSaved }: { config: ConfigFile; onSaved: () => Promise<void> }) {
  const saved = config.weather;
  const [enabled, setEnabled] = useState(saved?.enabled ?? false);
  const [place, setPlace] = useState<Pick<WeatherConfig, "place" | "lat" | "lon"> | null>(saved ? { place: saved.place, lat: saved.lat, lon: saved.lon } : null);
  const [units, setUnits] = useState<WeatherConfig["units"]>(saved?.units ?? "metric");
  const [query, setQuery] = useState("");
  const [found, setFound] = useState<PlaceResult[] | null>(null);
  const [busy, setBusy] = useState(false);
  const [message, setMessage] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  async function search() {
    setBusy(true);
    setError(null);
    try {
      const r = await call("search_place", { query });
      if ("error" in r) setError(r.error);
      else setFound(r);
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  }

  async function save(next: { enabled: boolean; place: typeof place; units: typeof units }) {
    setError(null);
    if (next.enabled && !next.place) return setError(t("weather.choose_first"));
    const weather: WeatherConfig | undefined = next.place ? { enabled: next.enabled, ...next.place, units: next.units } : undefined;
    const fresh = await call("get_config");
    const { weather: _old, ...rest } = fresh;
    const result = await call("save_config", { config: weather ? { ...rest, weather } : rest });
    if (!result.ok) return setError(result.issues[0]?.message ?? "");
    setMessage(t("common.saved"));
    await onSaved();
  }

  async function refresh() {
    setBusy(true);
    setError(null);
    try {
      const r = await call("refresh_weather");
      if ("error" in r) setError(r.error);
      else setMessage(t("weather.updated"));
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  }

  return (
    <section className="card" id="weather">
      <h3>{t("weather.title")}</h3>
      <label className="check">
        <input
          type="checkbox"
          checked={enabled}
          onChange={(e) => {
            setEnabled(e.target.checked);
            void save({ enabled: e.target.checked, place, units });
          }}
        />{" "}
        {t("weather.enable")}
      </label>
      <p className="muted">{place ? t("weather.place", { place: place.place }) : t("weather.no_place")}</p>
      <div className="row">
        <input
          className="grow"
          aria-label={t("weather.search")}
          placeholder={t("weather.search")}
          value={query}
          onChange={(e) => setQuery(e.target.value)}
          onKeyDown={(e) => e.key === "Enter" && void search()}
        />
        <Button disabled={busy || query.trim().length < 2} onClick={() => void search()}>{t("weather.find")}</Button>
      </div>
      {found !== null && found.length === 0 && <p className="muted">{t("weather.nothing")}</p>}
      {found !== null && found.length > 0 && (
        <ul className="plain-list place-list">
          {found.map((p, i) => (
            <li key={i}>
              <button
                type="button"
                className="picker-row"
                onClick={() => {
                  const next = { place: [p.name, p.country].filter(Boolean).join(", ").slice(0, 80), lat: p.lat, lon: p.lon };
                  setPlace(next);
                  setFound(null);
                  setQuery("");
                  void save({ enabled, place: next, units });
                }}
              >
                <span className="picker-label">{label(p)}</span>
              </button>
            </li>
          ))}
        </ul>
      )}
      <div className="row" role="radiogroup" aria-label={t("weather.units")}>
        <span className="muted">{t("weather.units")}</span>
        {(["metric", "imperial"] as const).map((u) => (
          <label key={u} className="check">
            <input
              type="radio"
              name="weather-units"
              checked={units === u}
              onChange={() => {
                setUnits(u);
                void save({ enabled, place, units: u });
              }}
            />{" "}
            {u === "metric" ? "°C" : "°F"}
          </label>
        ))}
        <span className="grow" />
        <Button disabled={busy || !enabled || !place} onClick={() => void refresh()}>{t("weather.refresh")}</Button>
      </div>
      {error && <Banner kind="error">{error}</Banner>}
      {message && !error && <small className="muted block">{message}</small>}
      <small className="muted block">{t("weather.credit")}</small>
    </section>
  );
}
