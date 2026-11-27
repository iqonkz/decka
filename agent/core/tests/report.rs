//! The problem report (T31.3) of a running agent with secrets everywhere: none of them is in the archive.

use std::sync::Arc;

use decka_core::actions::DryRunner;
use decka_core::agent::{Agent, AgentOptions};
use serde_json::json;

const MAIL_PASSWORD: &str = "SECRET-MAIL-PASSWORD-31";
const OBS_PASSWORD: &str = "SECRET-OBS-PASSWORD-31";
const MAIL_USER: &str = "somebody@example.org";

#[tokio::test(flavor = "multi_thread")]
async fn the_report_holds_no_token_password_or_blob() {
    let dir = tempfile::tempdir().unwrap();
    let mut config = decka_core::config::default_config();
    config.collectors.focus = Some(decka_core::model::SwitchConfig { enabled: false });
    config.collectors.media = Some(decka_core::model::SwitchConfig { enabled: false });
    config.obs = Some(serde_json::from_value(json!({ "enabled": false, "port": 1998 })).unwrap());
    config.mail = Some(serde_json::from_value(json!({ "accounts": [{ "id": "m1", "name": "Home box", "host": "localhost", "port": 1, "user": MAIL_USER }] })).unwrap());
    decka_core::config::save_json_atomic(&dir.path().join("config.json"), &config).unwrap();
    let agent = Agent::start(AgentOptions {
        bind_override: Some("127.0.0.1".to_string()),
        config_dir: Some(dir.path().to_path_buf()),
        client_dist: None,
        port_override: Some(0),
        runner: Arc::new(DryRunner),
        fixed_pair_code: None,
    })
    .await
    .unwrap();
    agent.set_obs_password(OBS_PASSWORD).unwrap();
    agent.set_mail_password("m1", MAIL_PASSWORD).unwrap();
    let saved = agent.config();
    let api_token = saved.api_token.clone();
    let blobs = [saved.obs.unwrap().password.unwrap(), saved.mail.unwrap().accounts[0].password.clone().unwrap()];

    // A log that holds everything it never should.
    let logs = decka_core::logging::logs_dir(dir.path());
    std::fs::create_dir_all(&logs).unwrap();
    let leaky = format!("old line\nINFO leak {api_token} {} {}\nINFO who {MAIL_USER}\nINFO end of the log\n", blobs[0], blobs[1]);
    let many: String = (0..700).map(|i| format!("INFO filler number {i}\n")).collect();
    std::fs::write(logs.join("decka.log"), format!("{many}{leaky}")).unwrap();
    let voice_log = dir.path().join("voice.log");
    std::fs::write(&voice_log, format!("1 note {api_token}\n2 stage listening\n")).unwrap();

    let out = tempfile::tempdir().unwrap();
    let request = decka_core::report::Request {
        out_dir: out.path().to_path_buf(),
        voice_log: Some(voice_log),
        voice_status: json!({ "state": "ready", "model": "standard", "mics": ["Somebody's headset"] }),
        voice_check: json!([{ "id": "stt", "state": "ok", "detail": "the words that were said" }]),
        webview: Some("140.0".into()),
    };
    let first = decka_core::report::save(&agent, &request).unwrap();
    let second = decka_core::report::save(&agent, &request).unwrap();
    assert_ne!(first, second, "an earlier report is not overwritten");
    agent.shutdown().await;

    let files = decka_core::zipfile::read(&std::fs::read(&first).unwrap(), 10 << 20).unwrap();
    let names: Vec<&str> = files.iter().map(|(n, _)| n.as_str()).collect();
    assert_eq!(names, ["system.json", "config.json", "voice-check.json", "decka.log", "voice.log"]);
    let text: String = files.iter().map(|(_, d)| String::from_utf8_lossy(d).to_string()).collect();
    for (what, secret) in [
        ("API token", api_token.as_str()),
        ("OBS password", OBS_PASSWORD),
        ("mail password", MAIL_PASSWORD),
        ("OBS blob", blobs[0].as_str()),
        ("mail blob", blobs[1].as_str()),
        ("any DPAPI blob", "dpapi:"),
        ("mail user", MAIL_USER),
        ("name of a mailbox", "Home box"),
        ("microphone name", "headset"),
        ("heard words", "words that were said"),
    ] {
        assert!(!text.contains(secret), "the report holds the {what}");
    }
    let log = String::from_utf8_lossy(&files[3].1).to_string();
    assert_eq!(log.lines().count(), decka_core::report::LOG_LINES);
    assert!(log.contains("end of the log") && !log.contains("filler number 3\n"));
    assert!(text.contains("\"state\": \"ready\"") && text.contains(env!("CARGO_PKG_VERSION")));
}
