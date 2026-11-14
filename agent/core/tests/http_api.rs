use std::sync::Arc;

use decka_core::actions::DryRunner;
use decka_core::{Agent, AgentOptions};
use serde_json::{json, Value};

const FIXTURES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../shared/fixtures");

struct Env {
    agent: Agent,
    base: String,
    token: String,
    client: reqwest::Client,
    _dir: tempfile::TempDir,
}

async fn env() -> Env {
    let dir = tempfile::tempdir().unwrap();
    let agent = Agent::start(AgentOptions {
        config_dir: Some(dir.path().to_path_buf()),
        client_dist: None,
        port_override: Some(0),
        runner: Arc::new(DryRunner),
        fixed_pair_code: None,
    })
    .await
    .unwrap();
    let base = format!("http://127.0.0.1:{}", agent.status().listening.unwrap().port);
    let token = agent.config().api_token;
    Env { agent, base, token, client: reqwest::Client::new(), _dir: dir }
}

fn fixture(name: &str) -> String {
    std::fs::read_to_string(format!("{FIXTURES}/{name}.json")).unwrap()
}

impl Env {
    fn post(&self, path: &str) -> reqwest::RequestBuilder {
        self.client.post(format!("{}{path}", self.base))
    }
    fn authed(&self, path: &str) -> reqwest::RequestBuilder {
        self.post(path).header("Authorization", format!("Bearer {}", self.token))
    }
    fn section(&self, name: &str) -> Value {
        self.agent.store().get(name)
    }
}

async fn json_of(res: reqwest::Response) -> (u16, Value) {
    let status = res.status().as_u16();
    let text = res.text().await.unwrap();
    (status, serde_json::from_str(&text).unwrap_or(Value::Null))
}

#[tokio::test]
async fn notify_without_or_with_a_wrong_token_is_401() {
    let e = env().await;
    let (status, body) = json_of(e.post("/api/notify").body("x").send().await.unwrap()).await;
    assert_eq!(status, 401);
    assert_eq!(body["error"]["code"], "UNAUTHORIZED");
    let res = e.post("/api/notify").header("Authorization", "Bearer dk_wrong").body("x").send().await.unwrap();
    assert_eq!(res.status(), 401);
    e.agent.shutdown().await;
}

#[tokio::test]
async fn a_token_in_the_query_string_is_not_accepted() {
    let e = env().await;
    let res = e.post(&format!("/api/notify?token={}", e.token)).body("x").send().await.unwrap();
    assert_eq!(res.status(), 401);
    e.agent.shutdown().await;
}

#[tokio::test]
async fn json_fixture_is_accepted_and_goes_first() {
    let e = env().await;
    let res = e.authed("/api/notify").header("Content-Type", "application/json").body(fixture("http-notify-req")).send().await.unwrap();
    let (status, body) = json_of(res).await;
    assert_eq!((status, body), (202, json!({ "id": "1" })));
    let list = e.section("notifications");
    assert_eq!(list[0]["title"], "Deploy finished");
    assert_eq!(list[0]["level"], "success");
    assert_eq!(list[0]["source"], "ci");
    e.agent.shutdown().await;
}

#[tokio::test]
async fn a_leading_bom_is_stripped_from_the_body() {
    let e = env().await;
    let mut body = vec![0xEF, 0xBB, 0xBF];
    body.extend_from_slice(r#"{"title":"Тест"}"#.as_bytes());
    let res = e.authed("/api/notify").header("Content-Type", "application/json").body(body).send().await.unwrap();
    assert_eq!(res.status(), 202);
    assert_eq!(e.section("notifications")[0]["title"], "Тест");
    e.agent.shutdown().await;
}

#[tokio::test]
async fn plain_text_becomes_the_title_with_level_info() {
    let e = env().await;
    let res = e.authed("/api/notify").header("Content-Type", "text/plain").body("  hello \n").send().await.unwrap();
    assert_eq!(res.status(), 202);
    let list = e.section("notifications");
    assert_eq!(list[0]["title"], "hello");
    assert_eq!(list[0]["level"], "info");

    // No Content-Type at all is treated as plain text; long text is cut to 120 characters.
    let res = e.authed("/api/notify").body("y".repeat(300)).send().await.unwrap();
    assert_eq!(res.status(), 202);
    assert_eq!(e.section("notifications")[0]["title"].as_str().unwrap().len(), 120);

    let res = e.authed("/api/notify").body("   ").send().await.unwrap();
    assert_eq!(res.status(), 400);
    e.agent.shutdown().await;
}

#[tokio::test]
async fn limits_are_enforced_without_truncation() {
    let e = env().await;
    let send = |body: Value| e.authed("/api/notify").header("Content-Type", "application/json").body(body.to_string()).send();
    for bad in [
        json!({ "title": "t".repeat(121) }),
        json!({ "title": "" }),
        json!({ "title": "ok", "body": "b".repeat(501) }),
        json!({ "title": "ok", "source": "s".repeat(41) }),
        json!({ "title": "ok", "level": "loud" }),
        json!({ "nope": 1 }),
    ] {
        let (status, body) = json_of(send(bad.clone()).await.unwrap()).await;
        assert_eq!((status, body["error"]["code"].as_str()), (400, Some("BAD_MESSAGE")), "{bad}");
    }
    assert_eq!(e.section("notifications"), json!([]));
    assert_eq!(send(json!({ "title": "t".repeat(120) })).await.unwrap().status(), 202);
    e.agent.shutdown().await;
}

#[tokio::test]
async fn the_feed_keeps_twenty_and_drops_the_oldest() {
    let e = env().await;
    for i in 0..21 {
        let res = e.authed("/api/notify").header("Content-Type", "application/json").body(json!({ "title": format!("n{i}") }).to_string()).send().await.unwrap();
        assert_eq!(res.status(), 202);
    }
    let list = e.section("notifications");
    let titles: Vec<&str> = list.as_array().unwrap().iter().map(|n| n["title"].as_str().unwrap()).collect();
    assert_eq!(titles.len(), 20);
    assert_eq!(titles[0], "n20");
    assert!(!titles.contains(&"n0"));
    e.agent.shutdown().await;
}

#[tokio::test]
async fn the_sixty_first_request_in_a_minute_is_429() {
    let e = env().await;
    for _ in 0..60 {
        assert_eq!(e.authed("/api/notify").body("x").send().await.unwrap().status(), 202);
    }
    let (status, body) = json_of(e.authed("/api/notify").body("x").send().await.unwrap()).await;
    assert_eq!((status, body["error"]["code"].as_str()), (429, Some("RATE_LIMITED")));
    e.agent.shutdown().await;
}

#[tokio::test]
async fn ten_unauthorized_requests_lock_the_ip_out() {
    let e = env().await;
    for _ in 0..10 {
        assert_eq!(e.post("/api/notify").body("x").send().await.unwrap().status(), 401);
    }
    assert_eq!(e.post("/api/notify").body("x").send().await.unwrap().status(), 429);
    assert_eq!(e.authed("/api/notify").body("x").send().await.unwrap().status(), 429);
    e.agent.shutdown().await;
}

#[tokio::test]
async fn a_twenty_kilobyte_body_is_413() {
    let e = env().await;
    let (status, body) = json_of(e.authed("/api/notify").header("Content-Type", "text/plain").body("z".repeat(20 * 1024)).send().await.unwrap()).await;
    assert_eq!((status, body["error"]["code"].as_str()), (413, Some("PAYLOAD_TOO_LARGE")));
    e.agent.shutdown().await;
}

// ===== T5.2: statuses and variables =====

impl Env {
    fn delete(&self, path: &str) -> reqwest::RequestBuilder {
        self.client.delete(format!("{}{path}", self.base)).header("Authorization", format!("Bearer {}", self.token))
    }
    fn put(&self, path: &str) -> reqwest::RequestBuilder {
        self.client.put(format!("{}{path}", self.base)).header("Authorization", format!("Bearer {}", self.token))
    }
}

fn json_req(b: reqwest::RequestBuilder, body: impl ToString) -> reqwest::RequestBuilder {
    b.header("Content-Type", "application/json").body(body.to_string())
}

#[tokio::test]
async fn status_upsert_from_the_fixture_is_a_push_item() {
    let e = env().await;
    let (status, body) = json_of(json_req(e.authed("/api/status"), fixture("http-status-req")).send().await.unwrap()).await;
    assert_eq!((status, body), (200, json!({ "ok": true })));
    let item = e.section("statuses")[0].clone();
    assert_eq!(item["id"], "claude-decka");
    assert_eq!(item["label"], "Claude: Decka");
    assert_eq!(item["source"], "push");
    assert_eq!(item["text"], "editing files");
    e.agent.shutdown().await;
}

#[tokio::test]
async fn upsert_keeps_label_and_clears_text() {
    let e = env().await;
    json_req(e.authed("/api/status"), fixture("http-status-req")).send().await.unwrap();
    json_req(e.authed("/api/status"), json!({ "id": "claude-decka", "state": "ok" })).send().await.unwrap();
    let item = e.section("statuses")[0].clone();
    assert_eq!(item["label"], "Claude: Decka");
    assert_eq!(item["group"], "ai");
    assert_eq!(item["state"], "ok");
    assert!(item.get("text").is_none());
    e.agent.shutdown().await;
}

#[tokio::test]
async fn ttl_makes_a_status_stale() {
    let e = env().await;
    json_req(e.authed("/api/status"), json!({ "id": "svc", "state": "ok", "text": "fine", "ttlSec": 1 })).send().await.unwrap();
    let mut stale = false;
    for _ in 0..30 {
        let item = e.section("statuses")[0].clone();
        if item["state"] == "unknown" && item["text"] == "stale" {
            stale = true;
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }
    assert!(stale, "status did not become stale within 3 s");
    e.agent.shutdown().await;
}

#[tokio::test]
async fn status_delete_then_404() {
    let e = env().await;
    json_req(e.authed("/api/status"), json!({ "id": "svc", "state": "ok" })).send().await.unwrap();
    assert_eq!(e.delete("/api/status/svc").send().await.unwrap().status(), 200);
    assert_eq!(e.section("statuses"), json!([]));
    assert_eq!(e.delete("/api/status/svc").send().await.unwrap().status(), 404);
    e.agent.shutdown().await;
}

#[tokio::test]
async fn invalid_status_requests_are_400() {
    let e = env().await;
    for bad in [
        json!({ "id": "Bad Id", "state": "ok" }),
        json!({ "id": "a", "state": "great" }),
        json!({ "id": "a", "state": "ok", "ttlSec": 0 }),
        json!({ "id": "a", "state": "ok", "ttlSec": 86401 }),
        json!({ "id": "a", "state": "ok", "text": "t".repeat(81) }),
        json!({ "id": "a", "state": "ok", "label": "" }),
    ] {
        let res = json_req(e.authed("/api/status"), bad.clone()).send().await.unwrap();
        assert_eq!(res.status(), 400, "{bad}");
    }
    e.agent.shutdown().await;
}

#[tokio::test]
async fn vars_put_get_delete() {
    let e = env().await;
    let res = json_req(e.put("/api/vars/mic.muted"), fixture("http-var-req")).send().await.unwrap();
    assert_eq!(res.status(), 200);
    assert_eq!(e.section("vars"), json!({ "mic.muted": true }));
    assert_eq!(e.delete("/api/vars/mic.muted").send().await.unwrap().status(), 200);
    assert_eq!(e.section("vars"), json!({}));
    assert_eq!(e.delete("/api/vars/mic.muted").send().await.unwrap().status(), 404);
    e.agent.shutdown().await;
}

#[tokio::test]
async fn invalid_var_requests_are_400() {
    let e = env().await;
    assert_eq!(json_req(e.put("/api/vars/Bad%20Name"), json!({ "value": 1 })).send().await.unwrap().status(), 400);
    assert_eq!(json_req(e.put("/api/vars/x"), json!({ "value": { "a": 1 } })).send().await.unwrap().status(), 400);
    assert_eq!(json_req(e.put("/api/vars/x"), json!({ "value": "s".repeat(201) })).send().await.unwrap().status(), 400);
    assert_eq!(json_req(e.put("/api/vars/x"), json!({ "value": "s".repeat(200) })).send().await.unwrap().status(), 200);
    e.agent.shutdown().await;
}

#[tokio::test]
async fn the_same_upsert_twice_bumps_rev_once() {
    let e = env().await;
    let rev = e.agent.store().rev();
    for _ in 0..2 {
        json_req(e.authed("/api/status"), fixture("http-status-req")).send().await.unwrap();
    }
    assert_eq!(e.agent.store().rev(), rev + 1);
    e.agent.shutdown().await;
}
