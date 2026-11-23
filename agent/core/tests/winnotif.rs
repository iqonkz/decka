//! Windows notifications on the phone (DATA_MODEL.md 10.4) without Windows: toasts are handed to the agent as if read, and
//! `notif_open` is checked against a recording runner (nothing real is launched, no real notification is read).

use std::sync::Arc;

use decka_core::actions::testing::RecordingRunner;
use decka_core::config::{default_config, save_json_atomic};
use decka_core::model::{NotifPrivacy, Notification, PrivacyMode, SwitchConfig, WindowsNotificationsConfig};
use decka_core::winnotif::WinToast;
use decka_core::{Agent, AgentOptions};

async fn start(dir: &std::path::Path, runner: Arc<RecordingRunner>, exclude: &[&str], privacy: Option<NotifPrivacy>) -> Agent {
    let mut config = default_config();
    for switch in [&mut config.collectors.audio, &mut config.collectors.media, &mut config.collectors.focus] {
        *switch = Some(SwitchConfig { enabled: false });
    }
    // The switch stays off: the reader thread is not started, the test hands toasts over itself.
    config.windows_notifications = Some(WindowsNotificationsConfig { enabled: false, exclude: exclude.iter().map(|s| s.to_string()).collect(), privacy });
    save_json_atomic(&dir.join("config.json"), &config).unwrap();
    Agent::start(AgentOptions { bind_override: Some("127.0.0.1".to_string()), config_dir: Some(dir.to_path_buf()), client_dist: None, port_override: Some(0), runner, fixed_pair_code: None }).await.unwrap()
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
    let agent = start(dir.path(), runner.clone(), &[], None).await;

    agent.windows_toast_for_test(toast(7, "Calendar", "Microsoft.WindowsCalendar_8wekyb3d8bbwe!App"));
    let list = notifications(&agent);
    assert_eq!(list.len(), 1);
    assert_eq!((list[0].source.as_deref(), list[0].app.as_deref(), list[0].openable, list[0].hidden), (Some("windows"), Some("Calendar"), Some(true), None));
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
    let agent = start(dir.path(), runner.clone(), &["spotify"], None).await;

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

#[tokio::test]
async fn a_messenger_notification_goes_out_without_its_text_in_the_mode_app() {
    let dir = tempfile::tempdir().unwrap();
    let agent = start(dir.path(), Arc::new(RecordingRunner::new()), &[], None).await;

    // Telegram is `app` by default: the program name only, no body, `hidden`; the words stay on the PC.
    agent.windows_toast_for_test(toast(1, "Telegram Desktop", "Telegram.exe"));
    let list = notifications(&agent);
    assert_eq!((list[0].title.as_str(), list[0].body.as_deref(), list[0].hidden), ("Telegram Desktop", None, Some(true)));
    let on_the_wire = serde_json::to_string(&agent.store().get("notifications")).unwrap();
    assert!(!on_the_wire.contains("Line one") && !on_the_wire.contains("Title"), "{on_the_wire}");
    // The tap still opens the program.
    assert_eq!(list[0].openable, Some(true));
}

#[tokio::test]
async fn the_owner_can_send_full_text_or_hide_any_program() {
    let dir = tempfile::tempdir().unwrap();
    let mut privacy = NotifPrivacy { default: PrivacyMode::Full, apps: Default::default() };
    privacy.apps.insert("Telegram Desktop".to_string(), PrivacyMode::Full);
    privacy.apps.insert("Calendar".to_string(), PrivacyMode::App);
    let agent = start(dir.path(), Arc::new(RecordingRunner::new()), &[], Some(privacy)).await;

    agent.windows_toast_for_test(toast(1, "Telegram Desktop", "Telegram.exe"));
    agent.windows_toast_for_test(toast(2, "Calendar", "Cal.exe"));
    let list = notifications(&agent);
    // Newest first: Calendar was set to `app`, Telegram to `full`.
    assert_eq!((list[0].title.as_str(), list[0].hidden), ("Calendar", Some(true)));
    assert_eq!((list[1].title.as_str(), list[1].body.as_deref(), list[1].hidden), ("Title", Some("Line one\nLine two"), None));
}
