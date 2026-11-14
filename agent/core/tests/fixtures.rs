//! Round-trip every shared fixture through the Rust types: JSON -> type -> JSON must be identical.

use decka_core::model::*;
use decka_core::proto::*;
use serde::{de::DeserializeOwned, Serialize};
use serde_json::Value;

const DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../shared/fixtures");

fn load(name: &str) -> Value {
    let text = std::fs::read_to_string(format!("{DIR}/{name}.json")).unwrap();
    serde_json::from_str(&text).unwrap()
}

fn roundtrip_value<T: Serialize + DeserializeOwned>(label: &str, original: Value) {
    let typed: T = serde_json::from_value(original.clone()).unwrap_or_else(|e| panic!("{label}: {e}"));
    let back = serde_json::to_value(&typed).unwrap();
    assert_eq!(back, original, "round-trip mismatch for {label}");
}

fn roundtrip<T: Serialize + DeserializeOwned>(name: &str) {
    roundtrip_value::<T>(name, load(name));
}

#[test]
fn client_messages() {
    for n in ["c2s-pair", "c2s-hello", "c2s-press", "c2s-notif-dismiss", "c2s-ping"] {
        roundtrip::<ClientMsg>(n);
    }
}

#[test]
fn server_messages() {
    for n in [
        "s2c-paired",
        "s2c-welcome",
        "s2c-patch-vars",
        "s2c-patch-focus",
        "s2c-patch-media",
        "s2c-press-result-ok",
        "s2c-press-result-err",
        "s2c-pong",
        "s2c-error",
    ] {
        roundtrip::<ServerMsg>(n);
    }
}

#[test]
fn welcome_state_sections() {
    let welcome = load("s2c-welcome");
    let state = &welcome["state"];
    roundtrip_value::<ClientLayout>("welcome.state.layout", state["layout"].clone());
    roundtrip_value::<SysStats>("welcome.state.sys", state["sys"].clone());
}

#[test]
fn m11_sections_have_typed_shapes() {
    roundtrip_value::<FocusInfo>("focus", load("s2c-patch-focus")["value"].clone());
    roundtrip_value::<MediaInfo>("media", load("s2c-patch-media")["value"].clone());
    roundtrip_value::<FocusInfo>("focus null app", serde_json::json!({ "app": null }));
}

#[test]
fn http_bodies() {
    roundtrip::<NotifyReq>("http-notify-req");
    roundtrip::<StatusReq>("http-status-req");
    roundtrip::<VarReq>("http-var-req");
    roundtrip::<HttpError>("http-error");
}

#[test]
fn files() {
    roundtrip::<ActionsFile>("actions.default");
    roundtrip::<LayoutFile>("layout.default");
    roundtrip::<LayoutFile>("layout.context");
    roundtrip::<ConfigFile>("config.example");
}
