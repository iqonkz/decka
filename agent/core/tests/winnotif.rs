//! Windows notifications on the phone (DATA_MODEL.md 10.4) without Windows: toasts are handed to the agent as if read, and
//! `notif_open` is checked against a recording runner (nothing real is launched, no real notification is read).

use std::sync::Arc;

use decka_core::actions::testing::RecordingRunner;
use decka_core::config::{default_config, save_json_atomic};
use decka_core::model::{Notification, SwitchConfig, WindowsNotificationsConfig};
use decka_core::winnotif::WinToast;
use decka_core::{Agent, AgentOptions};

async fn start(dir: &std::path::Path, runner: Arc<RecordingRunner>, exclude: &[&str]) -> Agent {
    let mut config = default_config();
    for switch in [&mut config.collectors.audio, &mut config.collectors.media, &mut config.collectors.focus] {
        *switch = Some(SwitchConfig { enabled: false });
    }
    // The switch stays off: the reader thread is not started, the test hands toasts over itself.
    config.windows_notifications = Some(WindowsNotificationsConfig { enabled: false, exclude: exclude.iter().map(|s| s.to_string()).collect() });
    save_json_atomic(&dir.join("config.json"), &config).unwrap();
    Agent::start(AgentOptions { config_dir: Some(dir.to_path_buf()), client_dist: None, port_override: Some(0), runner, fixed_pair_code: None }).await.unwrap()
}

fn toast(id: u32, app: &str, aumid: &str) -> WinToast {
    WinToast { id, app: app.into(), aumid: aumid.into(), texts: vec!["Title".into(), "Line one".into(), "Line two".into()] }
}

fn notifications(agent: &Agent) -> Vec<Notification> {
    serde_json::from_value(agent.store().get("notifications")).unwrap()
}

#[tokio::test]
async fn a_toast_becomes_a_notification_that_can_be_opened_by_its_id() {
    let dir = tempfile::tempdir().unwrap();
    let runner = Arc::new(RecordingRunner::new());
    let agent = start(dir.path(), runner.clone(), &[]).await;

    agent.windows_toast_for_test(toast(7, "Telegram", "TelegramMessengerLLP.TelegramDesktop_t4vj0pshhgkwm!Telegram"));
    let list = notifications(&agent);
    assert_eq!(list.len(), 1);
    assert_eq!((list[0].source.as_deref(), list[0].app.as_deref(), list[0].openable), (Some("windows"), Some("Telegram"), Some(true)));
    assert_eq!((list[0].title.as_str(), list[0].body.as_deref()), ("Title", Some("Line one\nLine two")));

    // An id the agent never gave out: nothing is launched.
    assert!(!agent.notif_open("999").await);
    assert!(!agent.notif_open("../../etc").await);
    assert!(runner.calls().is_empty());

    // The id of the notification: the program is launched once.
    assert!(agent.notif_open(&list[0].id).await);
    assert_eq!(runner.calls(), ["notif-open"]);
}

#[tokio::test]
async fn an_app_in_the_exception_list_is_not_sent_and_a_toast_without_a_program_cannot_be_opened() {
    let dir = tempfile::tempdir().unwrap();
    let runner = Arc::new(RecordingRunner::new());
    let agent = start(dir.path(), runner.clone(), &["spotify"]).await;

    agent.windows_toast_for_test(toast(1, "Spotify", "Spotify.exe"));
    assert!(notifications(&agent).is_empty());
    // The app is still remembered, so that the window can offer it in the list.
    assert!(agent.windows_notifications_status().apps.contains(&"Spotify".to_string()));

    agent.windows_toast_for_test(toast(2, "Old tool", "bad;aumid"));
    let list = notifications(&agent);
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].openable, None);
    assert!(!agent.notif_open(&list[0].id).await);
    assert!(runner.calls().is_empty());
}
