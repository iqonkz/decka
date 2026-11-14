//! Status manager: pushed statuses with optional TTL (PROTOCOL.md 5) and, later, HTTP-check results.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use crate::auth::now_ms;
use crate::model::{StatusItem, StatusSource, StatusState};
use crate::proto::StatusReq;
use crate::state::Store;

struct Entry {
    item: StatusItem,
    expires: Option<Instant>,
}

/// Cheap to clone: all clones share the same entries.
#[derive(Clone)]
pub struct Statuses {
    store: Store,
    entries: Arc<Mutex<HashMap<String, Entry>>>,
}

impl Statuses {
    pub fn new(store: Store) -> Statuses {
        Statuses { store, entries: Arc::new(Mutex::new(HashMap::new())) }
    }

    /// Writes the whole list to the store; `Store::set` drops it if nothing changed.
    fn publish(&self, entries: &HashMap<String, Entry>) {
        let items: Vec<StatusItem> = entries.values().map(|e| e.item.clone()).collect();
        self.store.update_statuses(|list| *list = items);
    }

    /// `POST /api/status`. Err = the id belongs to a check (not changeable through the API).
    pub fn upsert(&self, req: StatusReq) -> Result<(), String> {
        self.upsert_at(req, Instant::now())
    }

    fn upsert_at(&self, req: StatusReq, now: Instant) -> Result<(), String> {
        let mut entries = self.entries.lock().unwrap();
        let expires = req.ttl_sec.map(|s| now + Duration::from_secs(u64::from(s)));
        match entries.get_mut(&req.id) {
            Some(existing) if existing.item.source == StatusSource::Check => {
                return Err("status is owned by an HTTP check".to_string());
            }
            Some(existing) => {
                let old = existing.item.clone();
                let item = &mut existing.item;
                item.state = req.state;
                item.text = req.text; // not provided = cleared
                if let Some(label) = req.label {
                    item.label = label;
                }
                if let Some(group) = req.group {
                    item.group = Some(group);
                }
                // updatedAt moves only on a real change, so identical upserts are not rebroadcast.
                if (old.state, &old.text, &old.label, &old.group) != (item.state, &item.text, &item.label, &item.group) {
                    item.updated_at = now_ms();
                }
                existing.expires = expires;
            }
            None => {
                let item = StatusItem {
                    label: req.label.unwrap_or_else(|| req.id.clone()),
                    id: req.id.clone(),
                    state: req.state,
                    text: req.text,
                    group: req.group,
                    source: StatusSource::Push,
                    updated_at: now_ms(),
                };
                entries.insert(req.id, Entry { item, expires });
            }
        }
        self.publish(&entries);
        Ok(())
    }

    /// Publishes the result of an HTTP check. Bypasses the rule that forbids changing `source: "check"`.
    /// `updatedAt` moves only when `state` or `text` change.
    pub fn set_check(&self, id: &str, label: &str, group: Option<&str>, state: StatusState, text: Option<String>) {
        let mut entries = self.entries.lock().unwrap();
        match entries.get_mut(id) {
            Some(e) if e.item.source == StatusSource::Check => {
                if e.item.state != state || e.item.text != text {
                    e.item.updated_at = now_ms();
                }
                e.item.state = state;
                e.item.text = text;
                e.item.label = label.to_string();
                e.item.group = group.map(str::to_string);
            }
            _ => {
                let item = StatusItem {
                    id: id.to_string(),
                    label: label.to_string(),
                    state,
                    text,
                    group: group.map(str::to_string),
                    source: StatusSource::Check,
                    updated_at: now_ms(),
                };
                entries.insert(id.to_string(), Entry { item, expires: None });
            }
        }
        self.publish(&entries);
    }

    /// Removes every check status whose id is not in `keep` (a check was deleted from the config).
    pub fn retain_checks(&self, keep: &std::collections::HashSet<String>) {
        let mut entries = self.entries.lock().unwrap();
        let before = entries.len();
        entries.retain(|id, e| e.item.source != StatusSource::Check || keep.contains(id));
        if entries.len() != before {
            self.publish(&entries);
        }
    }

    /// `DELETE /api/status/{id}`: false if there is no such pushed status.
    pub fn delete(&self, id: &str) -> bool {
        let mut entries = self.entries.lock().unwrap();
        if entries.get(id).is_none_or(|e| e.item.source == StatusSource::Check) {
            return false;
        }
        entries.remove(id);
        self.publish(&entries);
        true
    }

    /// Items whose TTL has run out become `unknown` / "stale"; the TTL is then cleared.
    pub fn expire(&self, now: Instant) {
        let mut entries = self.entries.lock().unwrap();
        let mut changed = false;
        for e in entries.values_mut() {
            if e.expires.is_some_and(|t| t <= now) {
                e.expires = None;
                e.item.state = StatusState::Unknown;
                e.item.text = Some("stale".to_string());
                e.item.updated_at = now_ms();
                changed = true;
            }
        }
        if changed {
            self.publish(&entries);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn req(v: serde_json::Value) -> StatusReq {
        serde_json::from_value(v).unwrap()
    }

    fn statuses() -> (Store, Statuses) {
        let store = Store::new(json!({}));
        (store.clone(), Statuses::new(store))
    }

    #[test]
    fn defaults_and_keeping_label_and_group() {
        let (store, s) = statuses();
        s.upsert(req(json!({ "id": "a", "state": "ok" }))).unwrap();
        assert_eq!(store.get("statuses")[0]["label"], "a");
        s.upsert(req(json!({ "id": "a", "state": "running", "label": "Alpha", "group": "g", "text": "x" }))).unwrap();
        s.upsert(req(json!({ "id": "a", "state": "ok" }))).unwrap();
        let item = &store.get("statuses")[0];
        assert_eq!((item["label"].as_str(), item["group"].as_str()), (Some("Alpha"), Some("g")));
        assert!(item.get("text").is_none(), "text must be cleared when not provided");
    }

    #[test]
    fn identical_upsert_does_not_bump_rev() {
        let (store, s) = statuses();
        s.upsert(req(json!({ "id": "a", "state": "ok", "text": "t" }))).unwrap();
        let rev = store.rev();
        s.upsert(req(json!({ "id": "a", "state": "ok", "text": "t" }))).unwrap();
        assert_eq!(store.rev(), rev);
    }

    #[test]
    fn ttl_expiry_marks_stale_once() {
        let (store, s) = statuses();
        let t0 = Instant::now();
        s.upsert_at(req(json!({ "id": "a", "state": "ok", "ttlSec": 5 })), t0).unwrap();
        s.expire(t0 + Duration::from_secs(4));
        assert_eq!(store.get("statuses")[0]["state"], "ok");
        s.expire(t0 + Duration::from_secs(5));
        assert_eq!(store.get("statuses")[0]["state"], "unknown");
        assert_eq!(store.get("statuses")[0]["text"], "stale");
        let rev = store.rev();
        s.expire(t0 + Duration::from_secs(60));
        assert_eq!(store.rev(), rev);
    }

    #[test]
    fn check_results_keep_updated_at_while_state_and_text_are_equal() {
        let (store, s) = statuses();
        s.set_check("c", "C", None, StatusState::Ok, Some("200 · 80 ms".into()));
        let first = store.get("statuses")[0]["updatedAt"].as_u64().unwrap();
        let rev = store.rev();
        std::thread::sleep(Duration::from_millis(5));
        s.set_check("c", "C", None, StatusState::Ok, Some("200 · 80 ms".into()));
        assert_eq!(store.rev(), rev, "an identical result must not be rebroadcast");
        assert_eq!(store.get("statuses")[0]["updatedAt"].as_u64().unwrap(), first);
    }

    #[test]
    fn deleted_checks_lose_their_status_but_pushed_ones_stay() {
        let (store, s) = statuses();
        s.set_check("gone", "Gone", None, StatusState::Ok, None);
        s.set_check("kept", "Kept", None, StatusState::Ok, None);
        s.upsert(req(json!({ "id": "pushed", "state": "ok" }))).unwrap();
        s.retain_checks(&["kept".to_string()].into_iter().collect());
        let ids: Vec<String> = store.get("statuses").as_array().unwrap().iter().map(|i| i["id"].as_str().unwrap().to_string()).collect();
        assert_eq!(ids, ["kept", "pushed"]);
    }

    #[test]
    fn the_api_cannot_overwrite_or_delete_a_check() {
        let (_, s) = statuses();
        s.set_check("c", "C", None, StatusState::Ok, None);
        assert!(s.upsert(req(json!({ "id": "c", "state": "error" }))).is_err());
        assert!(!s.delete("c"));
    }
    #[test]
    fn delete_removes_once() {
        let (store, s) = statuses();
        s.upsert(req(json!({ "id": "a", "state": "ok" }))).unwrap();
        assert!(s.delete("a"));
        assert!(!s.delete("a"));
        assert_eq!(store.get("statuses"), json!([]));
    }
}
