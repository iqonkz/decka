//! Status manager: pushed statuses with optional TTL (PROTOCOL.md 5) and, later, HTTP-check results.

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use crate::auth::now_ms;
use crate::model::{StatusItem, StatusSource, StatusState};
use crate::proto::StatusReq;
use crate::state::Store;

struct Entry {
    item: StatusItem,
    expires: Option<Instant>,
}

pub struct Statuses {
    store: Store,
    entries: Mutex<HashMap<String, Entry>>,
}

impl Statuses {
    pub fn new(store: Store) -> Statuses {
        Statuses { store, entries: Mutex::new(HashMap::new()) }
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
    fn delete_removes_once() {
        let (store, s) = statuses();
        s.upsert(req(json!({ "id": "a", "state": "ok" }))).unwrap();
        assert!(s.delete("a"));
        assert!(!s.delete("a"));
        assert_eq!(store.get("statuses"), json!([]));
    }
}
