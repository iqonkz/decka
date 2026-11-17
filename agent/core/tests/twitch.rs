//! Twitch against a local stand-in (no network, no real account): the sign-in by code, the numbers, the two keys, the refresh of an
//! old token, and that no token ever reaches the window or the section.

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use decka_core::actions::DryRunner;
use decka_core::config::{default_config, save_json_atomic};
use decka_core::model::{Action, ActionsFile, SwitchConfig, TwitchConfig, TwitchOp};
use decka_core::{Agent, AgentOptions, SaveResult};
use serde_json::{json, Value};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

type Handler = Arc<dyn Fn(&str) -> (u16, String) + Send + Sync>;

/// Answers every request with what `handler` says about the whole request text; returns the address and the list of requests.
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

async fn start(dir: &std::path::Path, twitch: Option<TwitchConfig>) -> Agent {
    let mut config = default_config();
    config.twitch = twitch;
    for switch in [&mut config.collectors.audio, &mut config.collectors.media, &mut config.collectors.focus] {
        *switch = Some(SwitchConfig { enabled: false });
    }
    save_json_atomic(&dir.join("config.json"), &config).unwrap();
    Agent::start(AgentOptions { config_dir: Some(dir.to_path_buf()), client_dist: None, port_override: Some(0), runner: Arc::new(DryRunner), fixed_pair_code: None }).await.unwrap()
}

fn cfg(client_id: &str) -> TwitchConfig {
    TwitchConfig { enabled: false, client_id: client_id.into(), login: None, user_id: None, tokens: None, tokens_set: None }
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

/// The fake of Twitch: the code is typed at the second look; the first access token is "old" and is refused once.
fn twitch_fake(polls: Arc<Mutex<u32>>, refreshed: Arc<Mutex<u32>>) -> Handler {
    Arc::new(move |req: &str| {
        let line = req.lines().next().unwrap_or("");
        let ok = |v: Value| (200, v.to_string());
        if line.contains("/oauth2/device") {
            return ok(json!({ "device_code": "DEV", "user_code": "ABCDEFGH", "verification_uri": "https://www.twitch.tv/activate?device-code=ABCDEFGH", "expires_in": 600, "interval": 1 }));
        }
        if line.contains("/oauth2/token") && req.contains("grant_type=refresh_token") {
            *refreshed.lock().unwrap() += 1;
            return ok(json!({ "access_token": "new-access", "refresh_token": "new-refresh" }));
        }
        if line.contains("/oauth2/token") {
            let mut n = polls.lock().unwrap();
            *n += 1;
            return if *n < 2 { (400, json!({ "status": 400, "message": "authorization_pending" }).to_string()) } else { ok(json!({ "access_token": "old-access", "refresh_token": "old-refresh" })) };
        }
        // The Helix answers: "old-access" is refused for the clip once (it has expired), every other call is fine.
        if line.contains("/clips") && req.contains("Bearer old-access") {
            return (401, json!({ "message": "Invalid OAuth token" }).to_string());
        }
        if line.contains("/users") {
            return ok(json!({ "data": [{ "id": "4242", "login": "streamer" }] }));
        }
        if line.contains("/streams") {
            return ok(json!({ "data": [{ "type": "live", "viewer_count": 57, "title": "Speedrun", "game_name": "Celeste", "started_at": "2026-11-17T10:00:00Z" }] }));
        }
        if line.contains("/channels/followers") {
            return ok(json!({ "total": 321, "data": [] }));
        }
        if line.contains("/clips") {
            return ok(json!({ "data": [{ "id": "Clip1", "edit_url": "https://clips.twitch.tv/Clip1/edit" }] }));
        }
        if line.contains("/chat/announcements") {
            return (204, String::new());
        }
        (404, json!({ "message": "no such thing" }).to_string())
    })
}

#[tokio::test]
async fn sign_in_by_code_numbers_keys_refresh_and_sign_out() {
    let dir = tempfile::tempdir().unwrap();
    let agent = start(dir.path(), Some(cfg("clientid123"))).await;
    let polls = Arc::new(Mutex::new(0));
    let refreshed = Arc::new(Mutex::new(0));
    let (url, seen) = fake(twitch_fake(polls.clone(), refreshed.clone())).await;
    agent.set_twitch_hosts(&url, &url);
    assert_eq!(agent.store().get("twitch"), Value::Null, "signed out: no section");

    // The sign-in: the window gets the code; after the second look of the agent the channel is signed in and the numbers flow.
    let login = agent.twitch_login_start().await.unwrap();
    assert_eq!(login.user_code, "ABCDEFGH");
    assert!(login.verification_uri.contains("twitch.tv/activate"));
    assert_eq!(agent.twitch_status().pending.as_ref().map(|p| p.user_code.as_str()), Some("ABCDEFGH"));
    assert!(until(10, || agent.store().get("twitch")["connected"] == true).await, "never connected: {:?} {:?}", agent.twitch_status(), agent.store().get("twitch"));
    let section = agent.store().get("twitch");
    assert_eq!((section["login"].as_str(), section["live"].clone(), section["viewers"].clone(), section["followers"].clone()), (Some("streamer"), json!(true), json!(57), json!(321)));
    assert_eq!(section["title"], "Speedrun");
    let vars = agent.store().get("vars");
    assert_eq!((vars["twitch.live"].clone(), vars["twitch.viewers"].clone(), vars["twitch.followers"].clone()), (json!(true), json!(57), json!(321)));
    let status = agent.twitch_status();
    assert!(status.signed_in && status.enabled && status.pending.is_none() && status.login.as_deref() == Some("streamer"));
    // The client id goes to Twitch as a form field, and nothing else secret is in the section.
    assert!(seen.lock().unwrap().iter().any(|r| r.contains("client_id=clientid123") && r.contains("scopes=")));
    assert!(!section.to_string().contains("access") && !section.to_string().contains("refresh"));

    // The tokens are on disk only as a DPAPI blob, and the window never gets them.
    let file = std::fs::read_to_string(dir.path().join("config.json")).unwrap();
    assert!(file.contains("dpapi:") && !file.contains("old-access") && !file.contains("old-refresh"), "{file}");
    let for_window = agent.config_for_window().twitch.unwrap();
    assert_eq!((for_window.tokens, for_window.tokens_set), (None, Some(true)));
    // Saving the window's copy keeps the sign-in; another client id drops it.
    assert_eq!(agent.save_config(agent.config_for_window()), SaveResult::Saved);
    assert!(agent.twitch_status().signed_in);

    // The keys: a clip (the old token is refused once and refreshed), and an announcement with its text.
    let mut actions = ActionsFile { version: 1, actions: Default::default() };
    actions.actions.insert("clip".into(), Action::Twitch { name: "Clip".into(), toggle_var: None, op: TwitchOp::Clip, message: None });
    actions.actions.insert("say".into(), Action::Twitch { name: "Say".into(), toggle_var: None, op: TwitchOp::Announce, message: Some("Hello chat".into()) });
    assert_eq!(agent.save_actions(actions), SaveResult::Saved);
    assert_eq!(agent.execute("clip").await.map_err(|e| e.1), Ok(()));
    assert_eq!(*refreshed.lock().unwrap(), 1, "the expired token was refreshed once");
    assert_eq!(agent.execute("say").await.map_err(|e| e.1), Ok(()));
    {
        let requests = seen.lock().unwrap();
        let announce = requests.iter().find(|r| r.contains("/chat/announcements")).expect("an announcement was sent");
        assert!(announce.contains("broadcaster_id=4242") && announce.contains("moderator_id=4242") && announce.contains("Bearer new-access") && announce.contains("Hello chat"), "{announce}");
    }

    // Sign out: the tokens are gone, the section and the variables too.
    agent.twitch_logout().unwrap();
    assert_eq!(agent.store().get("twitch"), Value::Null);
    assert!(agent.store().get("vars").as_object().unwrap().keys().all(|k| !k.starts_with("twitch.")));
    assert!(!agent.twitch_status().signed_in);
    assert!(agent.execute("clip").await.is_err());
    agent.shutdown().await;
}

#[tokio::test]
async fn a_refused_code_and_a_missing_client_id_are_said_plainly() {
    let dir = tempfile::tempdir().unwrap();
    let agent = start(dir.path(), None).await;
    assert_eq!(agent.twitch_login_start().await.unwrap_err(), "Enter the client id of your Twitch application first");
    agent.shutdown().await;

    let dir = tempfile::tempdir().unwrap();
    let agent = start(dir.path(), Some(cfg("clientid123"))).await;
    let (url, _) = fake(Arc::new(|_: &str| (400, json!({ "status": 400, "message": "invalid client" }).to_string()))).await;
    agent.set_twitch_hosts(&url, &url);
    assert_eq!(agent.twitch_login_start().await.unwrap_err(), "Twitch: invalid client");
    agent.shutdown().await;
}

#[tokio::test]
async fn twitch_actions_are_checked_and_the_variables_are_reserved() {
    let dir = tempfile::tempdir().unwrap();
    let agent = start(dir.path(), None).await;
    let bad = |op, message: Option<&str>| {
        let mut a = ActionsFile { version: 1, actions: Default::default() };
        a.actions.insert("t".into(), Action::Twitch { name: "T".into(), toggle_var: None, op, message: message.map(str::to_string) });
        a
    };
    for (op, message) in [(TwitchOp::Announce, None), (TwitchOp::Clip, Some("text")), (TwitchOp::Announce, Some(""))] {
        assert!(matches!(agent.save_actions(bad(op, message)), SaveResult::Rejected(_)), "{op:?} {message:?}");
    }
    assert_eq!(agent.save_actions(bad(TwitchOp::Announce, Some("Hi"))), SaveResult::Saved);
    assert!(decka_core::validate::is_reserved_var("twitch.viewers"));
    agent.shutdown().await;
}
