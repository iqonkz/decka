//! Weather against a local stand-in for Open-Meteo (no network): the section, the settings that switch it on, the place search.

use std::sync::Arc;

use decka_core::actions::DryRunner;
use decka_core::config::{default_config, save_json_atomic};
use decka_core::model::{SwitchConfig, WeatherConfig};
use decka_core::{Agent, AgentOptions};
use serde_json::{json, Value};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

const FORECAST: &str = include_str!("../../../shared/fixtures/openmeteo-forecast.json");
const GEOCODING: &str = include_str!("../../../shared/fixtures/openmeteo-geocoding.json");

/// Answers every request with `body`; returns the address and the first line of the last request.
async fn fake(status: u16, body: &'static str) -> (String, Arc<std::sync::Mutex<String>>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}/", listener.local_addr().unwrap());
    let seen = Arc::new(std::sync::Mutex::new(String::new()));
    let for_task = seen.clone();
    tokio::spawn(async move {
        loop {
            let Ok((mut socket, _)) = listener.accept().await else { return };
            let mut buf = vec![0u8; 4096];
            let n = socket.read(&mut buf).await.unwrap_or(0);
            *for_task.lock().unwrap() = String::from_utf8_lossy(&buf[..n]).lines().next().unwrap_or("").to_string();
            let reply = format!("HTTP/1.1 {status} X\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len());
            let _ = socket.write_all(reply.as_bytes()).await;
        }
    });
    (url, seen)
}

async fn start(dir: &std::path::Path, weather: Option<WeatherConfig>) -> Agent {
    let mut config = default_config();
    config.weather = weather;
    for switch in [&mut config.collectors.audio, &mut config.collectors.media, &mut config.collectors.focus] {
        *switch = Some(SwitchConfig { enabled: false });
    }
    save_json_atomic(&dir.join("config.json"), &config).unwrap();
    Agent::start(AgentOptions { config_dir: Some(dir.to_path_buf()), client_dist: None, port_override: Some(0), runner: Arc::new(DryRunner), fixed_pair_code: None }).await.unwrap()
}

fn almaty(enabled: bool) -> WeatherConfig {
    WeatherConfig { enabled, place: "Almaty, Kazakhstan".into(), lat: 43.25654, lon: 76.92848, units: "metric".into() }
}

#[tokio::test]
async fn off_means_a_null_section_and_a_refusal() {
    let dir = tempfile::tempdir().unwrap();
    let agent = start(dir.path(), None).await;
    assert_eq!(agent.store().get("weather"), Value::Null);
    assert_eq!(agent.refresh_weather().await, Err("Weather is off".to_string()));
    assert_eq!(agent.refresh_weather_at("http://127.0.0.1:1/").await, Err("Weather is not set up".to_string()));
    agent.shutdown().await;
}

#[tokio::test]
async fn a_refresh_fills_the_section_and_sends_only_rounded_coordinates() {
    let dir = tempfile::tempdir().unwrap();
    let agent = start(dir.path(), Some(almaty(false))).await; // off: no loop asks the real host behind the test's back
    let (url, seen) = fake(200, FORECAST).await;

    let info = agent.refresh_weather_at(&url).await.unwrap();
    assert_eq!((info.place.as_str(), info.current.temp, info.daily.len()), ("Almaty, Kazakhstan", 7, 3));
    let section = agent.store().get("weather");
    assert_eq!(section["current"], json!({ "temp": 7, "code": 3, "isDay": true, "wind": 11 }));
    let request = seen.lock().unwrap().clone();
    assert!(request.contains("latitude=43.26&longitude=76.93"), "{request}");
    assert!(!request.contains("Almaty") && !request.contains("43.256"), "{request}");
    agent.shutdown().await;
}

#[tokio::test]
async fn a_failed_or_strange_answer_changes_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let agent = start(dir.path(), Some(almaty(false))).await;
    let (good, _) = fake(200, FORECAST).await;
    agent.refresh_weather_at(&good).await.unwrap();
    let before = agent.store().get("weather");

    let (down, _) = fake(500, "oops").await;
    assert_eq!(agent.refresh_weather_at(&down).await, Err("HTTP 500".to_string()));
    let (odd, _) = fake(200, r#"{"hello":1}"#).await;
    assert!(agent.refresh_weather_at(&odd).await.is_err());
    assert_eq!(agent.store().get("weather"), before);
    agent.shutdown().await;
}

#[tokio::test]
async fn saving_the_settings_turns_the_loop_on_and_off() {
    let dir = tempfile::tempdir().unwrap();
    let agent = start(dir.path(), Some(almaty(false))).await;
    let mut on = agent.config();
    on.weather = Some(almaty(true));
    assert_eq!(agent.save_config(on), decka_core::SaveResult::Saved); // the loop starts (and may ask the real host: nothing here depends on the answer)
    let mut off = agent.config();
    off.weather = Some(almaty(false));
    assert_eq!(agent.save_config(off), decka_core::SaveResult::Saved);
    assert_eq!(agent.store().get("weather"), Value::Null);
    agent.shutdown().await;
}

#[tokio::test]
async fn the_place_search_gives_up_to_five_places_and_checks_the_query() {
    let dir = tempfile::tempdir().unwrap();
    let agent = start(dir.path(), None).await;
    let (url, seen) = fake(200, GEOCODING).await;
    let places = agent.search_place_at(&url, "  Alma Ata ").await.unwrap();
    assert_eq!(places.len(), 5);
    assert_eq!((places[0].name.as_str(), places[0].lat, places[0].lon), ("Almaty", 43.26, 76.93));
    let request = seen.lock().unwrap().clone();
    assert!(request.contains("name=Alma+Ata") && request.contains("count=5") && request.contains("language=en"), "{request}");

    assert!(agent.search_place_at(&url, "a").await.is_err());
    assert!(agent.search_place_at(&url, &"x".repeat(81)).await.is_err());
    agent.shutdown().await;
}
