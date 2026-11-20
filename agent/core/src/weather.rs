//! Weather (DATA_MODEL.md 6.3): only the agent asks Open-Meteo, the phone never goes to the internet. The hosts are built into
//! the code, the coordinates are rounded to 2 decimals (about 1 km) before they leave the PC, and data older than 3 hours is
//! no data. Parsing is separate from the network so that it is tested on saved answers.

use std::time::Duration;

use serde::Serialize;
use serde_json::Value;

use crate::model::{WeatherDay, WeatherInfo, WeatherNow};

pub const FORECAST_URL: &str = "https://api.open-meteo.com/v1/forecast";
pub const GEOCODING_URL: &str = "https://geocoding-api.open-meteo.com/v1/search";
/// The forecast is asked again this often (and at start, and when the settings are saved).
pub const REFRESH_EVERY: Duration = Duration::from_secs(30 * 60);
/// Data older than this is dropped: the section becomes `null`.
pub const MAX_AGE_MS: u64 = 3 * 60 * 60 * 1000;
const TIMEOUT: Duration = Duration::from_secs(10);

/// One result of the place search (`search_place`).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Place {
    pub name: String,
    pub country: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub region: Option<String>,
    pub lat: f64,
    pub lon: f64,
}

/// Two decimals: about a kilometre, and not more is sent.
pub fn round_coord(v: f64) -> f64 {
    (v * 100.0).round() / 100.0
}

/// The forecast request for a place; `base` is `FORECAST_URL` (the tests use a local server).
pub fn forecast_url(base: &str, lat: f64, lon: f64, units: &str) -> String {
    let mut url = url::Url::parse(base).expect("the forecast address is valid");
    {
        let mut q = url.query_pairs_mut();
        q.append_pair("latitude", &format!("{}", round_coord(lat)));
        q.append_pair("longitude", &format!("{}", round_coord(lon)));
        q.append_pair("current", "temperature_2m,weather_code,is_day,wind_speed_10m");
        q.append_pair("daily", "weather_code,temperature_2m_max,temperature_2m_min");
        q.append_pair("forecast_days", "3");
        q.append_pair("timezone", "auto");
        if units == "imperial" {
            q.append_pair("temperature_unit", "fahrenheit");
            q.append_pair("wind_speed_unit", "mph");
        }
    }
    url.to_string()
}

/// The place search request; `language` is a language code of the interface (anything else is asked for in English).
pub fn geocoding_url(base: &str, query: &str, language: &str) -> String {
    let mut url = url::Url::parse(base).expect("the geocoding address is valid");
    url.query_pairs_mut().append_pair("name", query).append_pair("count", "5").append_pair("language", if crate::tray_text::LANGUAGES.contains(&language) { language } else { "en" });
    url.to_string()
}

fn whole(v: &Value) -> Option<i64> {
    v.as_f64().filter(|n| n.is_finite()).map(|n| n.round() as i64)
}

/// The answer of the forecast request as the `weather` section. `None` when something that is needed is missing: three days,
/// the current values.
pub fn parse_forecast(body: &str, place: &str, units: &str, now_ms: u64) -> Option<WeatherInfo> {
    let v: Value = serde_json::from_str(body).ok()?;
    let cur = v.get("current")?;
    let current = WeatherNow {
        temp: whole(cur.get("temperature_2m")?)?,
        code: u32::try_from(cur.get("weather_code")?.as_i64()?).ok().filter(|c| *c <= 99)?,
        is_day: cur.get("is_day").and_then(|d| d.as_i64().map(|n| n != 0).or_else(|| d.as_bool()))?,
        wind: whole(cur.get("wind_speed_10m")?)?.max(0),
    };
    let daily = v.get("daily")?;
    let list = |key: &str| daily.get(key).and_then(|a| a.as_array());
    let (dates, codes, max, min) = (list("time")?, list("weather_code")?, list("temperature_2m_max")?, list("temperature_2m_min")?);
    if dates.len() < 3 || codes.len() < 3 || max.len() < 3 || min.len() < 3 {
        return None;
    }
    let mut days = Vec::new();
    for i in 0..3 {
        let date = dates[i].as_str()?;
        if date.len() != 10 {
            return None;
        }
        days.push(WeatherDay { date: date.to_string(), min: whole(&min[i])?, max: whole(&max[i])?, code: u32::try_from(codes[i].as_i64()?).ok().filter(|c| *c <= 99)? });
    }
    Some(WeatherInfo {
        place: place.chars().take(80).collect(),
        units: if units == "imperial" { "imperial" } else { "metric" }.to_string(),
        updated_at: now_ms,
        current,
        daily: days,
    })
}

/// The answer of the place search: up to 5 places with a name and a position (a place without a country keeps an empty one).
pub fn parse_places(body: &str) -> Vec<Place> {
    let Ok(v) = serde_json::from_str::<Value>(body) else { return Vec::new() };
    let Some(results) = v.get("results").and_then(|r| r.as_array()) else { return Vec::new() };
    results
        .iter()
        .filter_map(|r| {
            let name = r.get("name")?.as_str()?.trim();
            let (lat, lon) = (r.get("latitude")?.as_f64()?, r.get("longitude")?.as_f64()?);
            if name.is_empty() || !(-90.0..=90.0).contains(&lat) || !(-180.0..=180.0).contains(&lon) {
                return None;
            }
            let text = |key: &str| r.get(key).and_then(|x| x.as_str()).map(str::trim).filter(|s| !s.is_empty()).map(|s| s.chars().take(60).collect::<String>());
            Some(Place { name: name.chars().take(60).collect(), country: text("country").unwrap_or_default(), region: text("admin1"), lat: round_coord(lat), lon: round_coord(lon) })
        })
        .take(5)
        .collect()
}

/// `Almaty, Kazakhstan` for the settings: the name of a place as it is kept in `config.weather.place`.
pub fn place_label(place: &Place) -> String {
    let parts: Vec<&str> = [Some(place.name.as_str()), (!place.country.is_empty()).then_some(place.country.as_str())].into_iter().flatten().collect();
    parts.join(", ")
}

/// What stays of the data we have: older than 3 hours is nothing.
pub fn fresh(info: Option<WeatherInfo>, now_ms: u64) -> Option<WeatherInfo> {
    info.filter(|i| now_ms.saturating_sub(i.updated_at) <= MAX_AGE_MS)
}

/// One request; any error -> its text.
pub async fn fetch(url: &str) -> Result<String, String> {
    let client = reqwest::Client::builder().timeout(TIMEOUT).user_agent(format!("decka-agent/{}", crate::VERSION)).build().map_err(|e| e.to_string())?;
    let response = client.get(url).send().await.map_err(|e| e.without_url().to_string())?;
    if !response.status().is_success() {
        return Err(format!("HTTP {}", response.status().as_u16()));
    }
    response.text().await.map_err(|e| e.without_url().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    const FORECAST: &str = include_str!("../../../shared/fixtures/openmeteo-forecast.json");
    const GEOCODING: &str = include_str!("../../../shared/fixtures/openmeteo-geocoding.json");

    #[test]
    fn coordinates_go_out_with_two_decimals() {
        assert_eq!([round_coord(43.256543), round_coord(76.928), round_coord(-0.004), round_coord(-33.8688)], [43.26, 76.93, 0.0, -33.87]);
        let url = forecast_url(FORECAST_URL, 43.256543, 76.928481, "metric");
        assert!(url.contains("latitude=43.26&longitude=76.93&"), "{url}");
        assert!(!url.contains("43.2565") && !url.contains("76.9284"));
        // Nothing else about the user: no name of the place, no id.
        assert!(url.starts_with("https://api.open-meteo.com/v1/forecast?") && !url.contains("Almaty"));
    }

    #[test]
    fn the_request_asks_for_what_the_widget_shows_and_for_imperial_units_when_wanted() {
        let metric = forecast_url(FORECAST_URL, 1.0, 2.0, "metric");
        assert!(metric.contains("current=temperature_2m%2Cweather_code%2Cis_day%2Cwind_speed_10m") && metric.contains("forecast_days=3") && metric.contains("timezone=auto"));
        assert!(!metric.contains("fahrenheit"));
        let imperial = forecast_url(FORECAST_URL, 1.0, 2.0, "imperial");
        assert!(imperial.contains("temperature_unit=fahrenheit") && imperial.contains("wind_speed_unit=mph"));
    }

    #[test]
    fn the_search_asks_for_five_in_the_language_of_the_window() {
        let url = geocoding_url(GEOCODING_URL, "Alma Ata", "ru");
        assert_eq!(url, "https://geocoding-api.open-meteo.com/v1/search?name=Alma+Ata&count=5&language=ru");
        assert!(geocoding_url(GEOCODING_URL, "x", "de").ends_with("language=de"));
        assert!(geocoding_url(GEOCODING_URL, "x", "xx").ends_with("language=en"));
    }

    #[test]
    fn a_forecast_becomes_the_weather_section() {
        let info = parse_forecast(FORECAST, "Almaty, Kazakhstan", "metric", 1_794_650_000_000).unwrap();
        assert_eq!(serde_json::to_value(&info).unwrap(), serde_json::json!({
            "place": "Almaty, Kazakhstan", "units": "metric", "updatedAt": 1_794_650_000_000u64,
            "current": { "temp": 7, "code": 3, "isDay": true, "wind": 11 },
            "daily": [
                { "date": "2026-11-16", "min": 2, "max": 9, "code": 3 },
                { "date": "2026-11-17", "min": 1, "max": 6, "code": 61 },
                { "date": "2026-11-18", "min": -1, "max": 4, "code": 71 }
            ]
        }));
    }

    #[test]
    fn night_and_missing_pieces() {
        let night = FORECAST.replace("\"is_day\": 1", "\"is_day\": 0");
        assert!(!parse_forecast(&night, "x", "metric", 0).unwrap().current.is_day);
        let mut value: Value = serde_json::from_str(FORECAST).unwrap();
        value["daily"]["time"].as_array_mut().unwrap().pop();
        assert_eq!(parse_forecast(&value.to_string(), "x", "metric", 0), None, "two days are not enough");
        for broken in ["", "{}", "[]", "not json", r#"{"current":{},"daily":{}}"#] {
            assert_eq!(parse_forecast(broken, "x", "metric", 0), None, "{broken}");
        }
        let odd_code = FORECAST.replace("\"weather_code\": 3,", "\"weather_code\": 300,");
        assert_eq!(parse_forecast(&odd_code, "x", "metric", 0), None);
    }

    #[test]
    fn the_place_name_is_cut_to_80_characters() {
        assert_eq!(parse_forecast(FORECAST, &"я".repeat(200), "metric", 0).unwrap().place.chars().count(), 80);
    }

    #[test]
    fn places_come_with_a_country_a_region_and_rounded_coordinates_at_most_five() {
        let places = parse_places(GEOCODING);
        assert_eq!(places.len(), 5, "the sixth is dropped");
        assert_eq!(places[0], Place { name: "Almaty".into(), country: "Kazakhstan".into(), region: Some("Almaty".into()), lat: 43.26, lon: 76.93 });
        assert_eq!(place_label(&places[0]), "Almaty, Kazakhstan");
        assert_eq!(places[2].country, "", "a place without a country keeps an empty one");
        assert_eq!(place_label(&places[2]), "Almaty");
        assert_eq!(serde_json::to_value(&places[1]).unwrap(), serde_json::json!({ "name": "Almaty Region", "country": "Kazakhstan", "lat": 44.5, "lon": 78.0 }));
    }

    #[test]
    fn no_results_is_an_empty_list() {
        assert_eq!(parse_places(r#"{"generationtime_ms":0.3}"#), vec![]);
        assert_eq!(parse_places("garbage"), vec![]);
        assert_eq!(parse_places(r#"{"results":[{"name":"X","latitude":95,"longitude":0}]}"#), vec![]);
    }

    #[test]
    fn data_older_than_three_hours_is_nothing() {
        let info = parse_forecast(FORECAST, "x", "metric", 1_000_000).unwrap();
        assert!(fresh(Some(info.clone()), 1_000_000 + MAX_AGE_MS).is_some());
        assert_eq!(fresh(Some(info.clone()), 1_000_000 + MAX_AGE_MS + 1), None);
        assert_eq!(fresh(None, 5), None);
        // A clock that went back is not "old".
        assert!(fresh(Some(info), 500).is_some());
    }
}
