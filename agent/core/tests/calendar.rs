//! T32.2: the ICS link is a secret - after saving it, it is nowhere in the clear: not in `config.json`, not in what the
//! window or the phone get, not in a backup.

use std::sync::Arc;

use decka_core::actions::DryRunner;
use decka_core::agent::{Agent, AgentOptions};

/// The host does not exist: the reading fails, nothing leaves this computer.
const LINK: &str = "https://calendar.invalid/ical/SECRET-ICS-TOKEN-71/basic.ics";

#[cfg(windows)]
#[tokio::test(flavor = "multi_thread")]
async fn the_link_of_the_calendar_is_kept_only_under_dpapi() {
    let dir = tempfile::tempdir().unwrap();
    let agent = Agent::start(AgentOptions {
        config_dir: Some(dir.path().to_path_buf()),
        client_dist: None,
        port_override: Some(0),
        bind_override: Some("127.0.0.1".into()),
        runner: Arc::new(DryRunner),
        fixed_pair_code: None,
    })
    .await
    .unwrap();

    assert!(agent.calendar_set("http://calendar.invalid/a.ics").is_err(), "only https");
    agent.calendar_set(LINK).unwrap();

    let on_disk = std::fs::read_to_string(dir.path().join("config.json")).unwrap();
    assert!(on_disk.contains("dpapi:") && !on_disk.contains("SECRET-ICS-TOKEN-71"), "{on_disk}");
    let window = serde_json::to_string(&agent.config_for_window()).unwrap();
    let view = agent.calendar_view();
    for text in [window.clone(), view.to_string(), serde_json::to_string(&agent.store().get("calendar")).unwrap()] {
        assert!(!text.contains("SECRET-ICS-TOKEN-71") && !text.contains("dpapi:Q"), "{text}");
    }
    assert_eq!(view["masked"], "https://calendar.invalid/••••••••");
    assert_eq!(agent.config_for_window().calendar.map(|c| c.url), Some(String::new()), "the window knows only that there is one");
    let backup = decka_core::backup::export(&agent).unwrap();
    assert!(!String::from_utf8_lossy(&backup).contains("SECRET-ICS-TOKEN-71"));

    // Saving the settings form does not lose the link and cannot change it.
    let mut form = agent.config_for_window();
    form.calendar = None;
    assert_eq!(agent.save_config(form), decka_core::SaveResult::Saved);
    assert!(agent.calendar_view()["connected"].as_bool().unwrap());

    agent.calendar_set("").unwrap();
    assert!(!std::fs::read_to_string(dir.path().join("config.json")).unwrap().contains("calendar"));
    assert!(agent.store().get("calendar").is_null());
    agent.shutdown().await;
}
