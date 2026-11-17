//! State store: ten JSON sections (five from the start, `focus` and `media` from M11, `icons` from M12, `weather` and `obs` from M15), a global `rev`, broadcast of changes only (ARCHITECTURE.md 5.2).

use std::collections::{BTreeMap, HashMap};
use std::sync::{Arc, Mutex};

use serde_json::Value;
use tokio::sync::broadcast;

use crate::model::{Notification, StatusItem, VarValue};

pub const SECTIONS: [&str; 12] = ["layout", "sys", "statuses", "notifications", "vars", "focus", "media", "icons", "weather", "obs", "ai", "twitch"];
pub const MAX_NOTIFICATIONS: usize = 20;

/// A change to one section; `value` is always the complete new value of the section.
#[derive(Debug, Clone)]
pub struct Patch {
    pub rev: u64,
    pub section: &'static str,
    pub value: Arc<Value>,
}

struct Inner {
    sections: HashMap<&'static str, Value>,
    rev: u64,
}

#[derive(Clone)]
pub struct Store {
    inner: Arc<Mutex<Inner>>,
    tx: broadcast::Sender<Patch>,
}

fn section_name(section: &str) -> Option<&'static str> {
    SECTIONS.iter().copied().find(|s| *s == section)
}

impl Inner {
    fn set(&mut self, tx: &broadcast::Sender<Patch>, section: &'static str, value: Value) -> bool {
        if self.sections.get(section) == Some(&value) {
            return false;
        }
        self.sections.insert(section, value.clone());
        self.rev += 1;
        // No receivers is fine: the error only means nobody is listening.
        let _ = tx.send(Patch { rev: self.rev, section, value: Arc::new(value) });
        true
    }
}

impl Store {
    /// `layout` is the initial layout section; the other sections start empty (`sys` = null).
    pub fn new(layout: Value) -> Store {
        let mut sections = HashMap::new();
        sections.insert("layout", layout);
        sections.insert("sys", Value::Null);
        sections.insert("statuses", Value::Array(Vec::new()));
        sections.insert("notifications", Value::Array(Vec::new()));
        sections.insert("vars", Value::Object(Default::default()));
        sections.insert("focus", Value::Null);
        sections.insert("media", Value::Null);
        sections.insert("icons", Value::Object(Default::default()));
        sections.insert("weather", Value::Null);
        sections.insert("obs", Value::Null);
        sections.insert("ai", Value::Null);
        sections.insert("twitch", Value::Null);
        let (tx, _) = broadcast::channel(64);
        Store { inner: Arc::new(Mutex::new(Inner { sections, rev: 0 })), tx }
    }

    /// Replace a section. Returns false (no broadcast, `rev` unchanged) if the value is equal.
    pub fn set(&self, section: &str, value: Value) -> bool {
        let Some(name) = section_name(section) else {
            debug_assert!(false, "unknown section {section}");
            return false;
        };
        self.inner.lock().unwrap().set(&self.tx, name, value)
    }

    pub fn get(&self, section: &str) -> Value {
        self.inner.lock().unwrap().sections.get(section).cloned().unwrap_or(Value::Null)
    }

    /// (rev, FullState object with all five sections).
    pub fn snapshot(&self) -> (u64, Value) {
        let inner = self.inner.lock().unwrap();
        let map: serde_json::Map<String, Value> =
            SECTIONS.iter().map(|s| (s.to_string(), inner.sections.get(s).cloned().unwrap_or(Value::Null))).collect();
        (inner.rev, Value::Object(map))
    }

    pub fn rev(&self) -> u64 {
        self.inner.lock().unwrap().rev
    }

    pub fn subscribe(&self) -> broadcast::Receiver<Patch> {
        self.tx.subscribe()
    }

    pub fn receiver_count(&self) -> usize {
        self.tx.receiver_count()
    }

    /// Read-modify-write of a typed section under one lock.
    fn update<T, R>(&self, section: &'static str, f: impl FnOnce(&mut T) -> R, post: impl FnOnce(&mut T)) -> R
    where
        T: serde::Serialize + serde::de::DeserializeOwned + Default,
    {
        let mut inner = self.inner.lock().unwrap();
        let mut typed: T = inner.sections.get(section).and_then(|v| serde_json::from_value(v.clone()).ok()).unwrap_or_default();
        let result = f(&mut typed);
        post(&mut typed);
        let value = serde_json::to_value(&typed).expect("section serializes");
        inner.set(&self.tx, section, value);
        result
    }

    pub fn update_vars(&self, f: impl FnOnce(&mut BTreeMap<String, VarValue>)) {
        self.update("vars", f, |_| {});
    }

    /// After the closure the list is sorted by (group, id).
    pub fn update_statuses(&self, f: impl FnOnce(&mut Vec<StatusItem>)) {
        self.update("statuses", f, |v: &mut Vec<StatusItem>| {
            v.sort_by(|a, b| (&a.group, &a.id).cmp(&(&b.group, &b.id)));
        });
    }

    /// After the closure the list is truncated to 20 entries (newest first).
    pub fn update_notifications(&self, f: impl FnOnce(&mut Vec<Notification>)) {
        self.update("notifications", f, |v: &mut Vec<Notification>| v.truncate(MAX_NOTIFICATIONS));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{NotifLevel, StatusSource, StatusState};
    use serde_json::json;

    fn status(id: &str, group: Option<&str>) -> StatusItem {
        StatusItem {
            id: id.to_string(),
            label: id.to_string(),
            state: StatusState::Ok,
            text: None,
            group: group.map(str::to_string),
            source: StatusSource::Push,
            updated_at: 0,
        }
    }

    #[test]
    fn same_value_is_not_broadcast() {
        let store = Store::new(json!({}));
        let mut rx = store.subscribe();
        assert!(store.set("vars", json!({ "a": true })));
        assert!(rx.try_recv().is_ok());
        assert!(!store.set("vars", json!({ "a": true })));
        assert!(rx.try_recv().is_err());
        assert_eq!(store.rev(), 1);
    }

    #[test]
    fn two_sets_give_rev_1_and_2_in_order() {
        let store = Store::new(json!({}));
        let mut rx = store.subscribe();
        store.set("vars", json!({ "a": 1 }));
        store.set("sys", json!({ "cpu": 5 }));
        let a = rx.try_recv().unwrap();
        let b = rx.try_recv().unwrap();
        assert_eq!((a.rev, a.section), (1, "vars"));
        assert_eq!((b.rev, b.section), (2, "sys"));
        assert_eq!(*b.value, json!({ "cpu": 5 }));
    }

    #[test]
    fn snapshot_has_all_sections() {
        let store = Store::new(json!({ "pages": [] }));
        let (rev, state) = store.snapshot();
        assert_eq!(rev, 0);
        let obj = state.as_object().unwrap();
        for s in SECTIONS {
            assert!(obj.contains_key(s), "missing {s}");
        }
        assert_eq!(obj["sys"], Value::Null);
        assert_eq!((&obj["focus"], &obj["media"]), (&Value::Null, &Value::Null));
        assert_eq!(obj["icons"], json!({}));
        assert_eq!(obj["layout"], json!({ "pages": [] }));
    }

    #[test]
    fn update_statuses_sorts_by_group_then_id() {
        let store = Store::new(json!({}));
        store.update_statuses(|v| {
            v.push(status("b", Some("z")));
            v.push(status("c", None));
            v.push(status("a", Some("z")));
            v.push(status("d", Some("a")));
        });
        let ids: Vec<String> = serde_json::from_value::<Vec<StatusItem>>(store.get("statuses")).unwrap().into_iter().map(|s| s.id).collect();
        assert_eq!(ids, ["c", "d", "a", "b"]);
    }

    #[test]
    fn update_notifications_truncates_to_20() {
        let store = Store::new(json!({}));
        store.update_notifications(|v| {
            for i in 0..25 {
                v.push(Notification { id: i.to_string(), ts: 0, title: "t".into(), body: None, level: NotifLevel::Info, source: None });
            }
        });
        let list: Vec<Notification> = serde_json::from_value(store.get("notifications")).unwrap();
        assert_eq!(list.len(), 20);
        assert_eq!(list[0].id, "0");
    }
}
