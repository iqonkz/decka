//! YouTube against a local stand-in (no network, no real account): the secret, the sign-in by code, the numbers, the chat key, the
//! refresh of an old token, and that neither the secret nor a token reaches the window, the section or the file in plain text.

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use decka_core::actions::DryRunner;
use decka_core::config::{default_config, save_json_atomic};
use decka_core::model::{Action, ActionsFile, SwitchConfig, YoutubeConfig, YoutubeOp};
use decka_core::{Agent, AgentOptions, SaveResult};
use serde_json::{json, Value};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

type Handler = Arc<dyn Fn(&str) -> (u16, String) + Send + Sync>;

async fn fake(handler: Handler) -> (String, Arc<Mutex<Vec<String>>>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let seen = Arc::new(Mutex::new(Vec::new()));
    let for_task = seen.clone();
    tokio::spawn(async move {
        loop {
            let Ok((mut socket, _)) = listener.accept().await else { return };
            let mut buf = vec![0u8; 16384];
            let n = socket.read(&mut buf).await.unwrap_or(0);
            let request = String::from_utf8_lossy(&buf[..n]).to_string();
            for_task.lock().unwrap().push(request.clone());
            let (status, body) = handler(&request);
            let reply = format!("HTTP/1.1 {status} X\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len());
            let _ = socket.write_all(reply.as_bytes()).await;
        }
    });
    (url, seen)
}

async fn start(dir: &std::path::Path, youtube: Option<YoutubeConfig>) -> Agent {
    let mut config = default_config();
    config.youtube = youtube;
    for switch in [&mut config.collectors.audio, &mut config.collectors.media, &mut config.collectors.focus] {
        *switch = Some(SwitchConfig { enabled: false });
    }
    save_json_atomic(&dir.join("config.json"), &config).unwrap();
    Agent::start(AgentOptions { config_dir: Some(dir.to_path_buf()), client_dist: None, port_override: Some(0), runner: Arc::new(DryRunner), fixed_pair_code: None }).await.unwrap()
}

fn cfg(client_id: &str) -> YoutubeConfig {
    YoutubeConfig { enabled: false, client_id: client_id.into(), secret: None, channel: None, tokens: None, secret_set: None, tokens_set: None }
}

async fn until(secs: u64, mut done: impl FnMut() -> bool) -> bool {
    let end = Instant::now() + Duration::from_secs(secs);
    while Instant::now() < end {
        if done() {
            return true;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    done()
}

/// The fake of Google and YouTube: the code is typed at the second look; "old-access" is refused for the chat once.
fn fake_google(polls: Arc<Mutex<u32>>, refreshed: Arc<Mutex<u32>>) -> Handler {
    Arc::new(move |req: &str| {
        let line = req.lines().next().unwrap_or("");
        let ok = |v: Value| (200, v.to_string());
        if line.contains("/device/code") {
            return ok(json!({ "device_code": "DEV", "user_code": "ABCD-EFGH", "verification_url": "https://www.google.com/device", "expires_in": 1800, "interval": 1 }));
        }
        if line.contains("/token") && req.contains("grant_type=refresh_token") {
            *refreshed.lock().unwrap() += 1;
            return ok(json!({ "access_token": "new-access", "expires_in": 3600 }));
        }
        if line.contains("/token") {
            let mut n = polls.lock().unwrap();
            *n += 1;
            return if *n < 2 { (428, json!({ "error": "authorization_pending" }).to_string()) } else { ok(json!({ "access_token": "old-access", "refresh_token": "the-refresh" })) };
        }
        if line.contains("/liveChat/messages") && req.contains("Bearer old-access") {
            return (401, json!({ "error": { "code": 401, "message": "Invalid Credentials" } }).to_string());
        }
        if line.contains("/channels") {
            return ok(json!({ "items": [{ "snippet": { "title": "My channel" }, "statistics": { "subscriberCount": "1500", "hiddenSubscriberCount": false } }] }));
        }
        if line.contains("/liveBroadcasts") {
            return ok(json!({ "items": [{ "id": "vid1", "snippet": { "title": "Speedrun", "liveChatId": "chat1", "actualStartTime": "2026-11-17T10:00:00Z" } }] }));
        }
        if line.contains("/videos") {
            return ok(json!({ "items": [{ "liveStreamingDetails": { "concurrentViewers": "88" } }] }));
        }
        if line.contains("/liveChat/messages") {
            return ok(json!({ "id": "m1" }));
        }
        (404, json!({ "error": { "message": "no such thing" } }).to_string())
    })
}

#[tokio::test]
async fn secret_sign_in_by_code_numbers_chat_refresh_and_sign_out() {
    let dir = tempfile::tempdir().unwrap();
    let agent = start(dir.path(), Some(cfg("123-abc.apps.googleusercontent.com"))).await;
    let polls = Arc::new(Mutex::new(0));
    let refreshed = Arc::new(Mutex::new(0));
    let (url, seen) = fake(fake_google(polls, refreshed.clone())).await;
    agent.set_youtube_hosts(&url, &url);
    assert_eq!(agent.store().get("youtube"), Value::Null, "signed out: no section");

    // Without the client secret the sign-in does not start.
    assert_eq!(agent.youtube_login_start().await.unwrap_err(), "Enter the client secret of your Google application first");
    agent.set_youtube_secret("super-secret-value").unwrap();
    assert!(agent.youtube_status().secret_set);

    let login = agent.youtube_login_start().await.unwrap();
    assert_eq!(login.user_code, "ABCD-EFGH");
    assert!(until(10, || agent.store().get("youtube")["connected"] == true).await, "never connected: {:?} {:?}", agent.youtube_status(), agent.store().get("youtube"));
    let section = agent.store().get("youtube");
    assert_eq!((section["channel"].as_str(), section["live"].clone(), section["viewers"].clone(), section["subscribers"].clone(), section["title"].as_str()), (Some("My channel"), json!(true), json!(88), json!(1500), Some("Speedrun")));
    let vars = agent.store().get("vars");
    assert_eq!((vars["youtube.live"].clone(), vars["youtube.viewers"].clone(), vars["youtube.subscribers"].clone()), (json!(true), json!(88), json!(1500)));
    let status = agent.youtube_status();
    assert!(status.signed_in && status.enabled && status.pending.is_none() && status.channel.as_deref() == Some("My channel"));
    // The secret goes to Google as a form field of the token request, and nowhere into the section.
    assert!(seen.lock().unwrap().iter().any(|r| r.contains("client_secret=super-secret-value") && r.contains("device_code=DEV")));
    assert!(!section.to_string().contains("secret") && !section.to_string().contains("access"));

    // On disk and in the window: DPAPI blobs only.
    let file = std::fs::read_to_string(dir.path().join("config.json")).unwrap();
    assert!(file.contains("dpapi:") && !file.contains("super-secret-value") && !file.contains("old-access") && !file.contains("the-refresh"), "{file}");
    let for_window = agent.config_for_window().youtube.unwrap();
    assert_eq!((for_window.secret, for_window.tokens, for_window.secret_set, for_window.tokens_set), (None, None, Some(true), Some(true)));
    assert_eq!(agent.save_config(agent.config_for_window()), SaveResult::Saved);
    assert!(agent.youtube_status().signed_in && agent.youtube_status().secret_set, "saving the window's copy keeps the sign-in");

    // The chat key: the old token is refused once and refreshed.
    let mut actions = ActionsFile { version: 1, actions: Default::default() };
    actions.actions.insert("say".into(), Action::Youtube { name: "Say".into(), toggle_var: None, op: YoutubeOp::Chat, message: Some("Hello chat".into()) });
    assert_eq!(agent.save_actions(actions), SaveResult::Saved);
    assert_eq!(agent.execute("say").await.map_err(|e| e.1), Ok(()));
    assert_eq!(*refreshed.lock().unwrap(), 1, "the expired token was refreshed once");
    {
        let requests = seen.lock().unwrap();
        let sent = requests.iter().rfind(|r| r.contains("/liveChat/messages") && r.contains("Bearer new-access")).expect("the message was sent with the new token");
        assert!(sent.contains("liveChatId") && sent.contains("chat1") && sent.contains("Hello chat"), "{sent}");
        assert!(requests.iter().any(|r| r.contains("grant_type=refresh_token") && r.contains("refresh_token=the-refresh") && r.contains("client_secret=super-secret-value")));
    }

    // Go live / end the stream: the scheduled broadcast is moved to `live`, the live one to `complete`.
    let mut actions = ActionsFile { version: 1, actions: Default::default() };
    actions.actions.insert("golive".into(), Action::Youtube { name: "Go live".into(), toggle_var: None, op: YoutubeOp::Start, message: None });
    actions.actions.insert("endlive".into(), Action::Youtube { name: "End".into(), toggle_var: None, op: YoutubeOp::End, message: None });
    assert_eq!(agent.save_actions(actions), SaveResult::Saved);
    assert_eq!(agent.execute("golive").await.map_err(|e| e.1), Ok(()));
    assert_eq!(agent.execute("endlive").await.map_err(|e| e.1), Ok(()));
    {
        let requests = seen.lock().unwrap();
        assert!(requests.iter().any(|r| r.starts_with("GET ") && r.contains("broadcastStatus=upcoming")), "the scheduled stream was looked up");
        assert!(requests.iter().any(|r| r.starts_with("POST ") && r.contains("/liveBroadcasts/transition") && r.contains("broadcastStatus=live") && r.contains("id=vid1")));
        assert!(requests.iter().any(|r| r.starts_with("POST ") && r.contains("/liveBroadcasts/transition") && r.contains("broadcastStatus=complete") && r.contains("id=vid1")));
    }

    // Sign out: the tokens are gone (the secret stays for the next sign-in), the section and the variables too.
    agent.youtube_logout().unwrap();
    assert_eq!(agent.store().get("youtube"), Value::Null);
    assert!(agent.store().get("vars").as_object().unwrap().keys().all(|k| !k.starts_with("youtube.")));
    let status = agent.youtube_status();
    assert!(!status.signed_in && status.secret_set);
    assert!(agent.execute("say").await.is_err());
    agent.shutdown().await;
}

#[tokio::test]
async fn plain_errors_and_checked_actions() {
    let dir = tempfile::tempdir().unwrap();
    let agent = start(dir.path(), None).await;
    assert_eq!(agent.youtube_login_start().await.unwrap_err(), "Enter the client id of your Google application first");
    let chat = |message: Option<&str>| {
        let mut a = ActionsFile { version: 1, actions: Default::default() };
        a.actions.insert("c".into(), Action::Youtube { name: "C".into(), toggle_var: None, op: YoutubeOp::Chat, message: message.map(str::to_string) });
        a
    };
    for message in [None, Some(""), Some(&"x".repeat(201)[..])] {
        assert!(matches!(agent.save_actions(chat(message)), SaveResult::Rejected(_)), "{message:?}");
    }
    assert_eq!(agent.save_actions(chat(Some("Hi"))), SaveResult::Saved);
    // Go live and end take no text.
    let mut live = ActionsFile { version: 1, actions: Default::default() };
    live.actions.insert("g".into(), Action::Youtube { name: "G".into(), toggle_var: None, op: YoutubeOp::Start, message: Some("x".into()) });
    assert!(matches!(agent.save_actions(live), SaveResult::Rejected(_)));
    assert!(decka_core::validate::is_reserved_var("youtube.viewers"));
    agent.shutdown().await;

    // A refused client: Google's own words.
    let dir = tempfile::tempdir().unwrap();
    let agent = start(dir.path(), Some(cfg("client.apps"))).await;
    agent.set_youtube_secret("s").unwrap();
    let (url, _) = fake(Arc::new(|_: &str| (401, json!({ "error": "invalid_client", "error_description": "The OAuth client was not found." }).to_string()))).await;
    agent.set_youtube_hosts(&url, &url);
    assert_eq!(agent.youtube_login_start().await.unwrap_err(), "YouTube: The OAuth client was not found.");
    agent.shutdown().await;
}
