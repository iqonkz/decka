//! T29.6: a button by voice. Only the ready-made keys of the catalog (`shared/catalog/catalog.json`) can be made: voice
//! never brings a program, keys or a URL of its own. A key that asks for something is set up in the settings window. The new
//! button goes into the first free cell of a page; the files are written only after a "yes", through the checked save.

use std::collections::{BTreeMap, HashSet};

use serde_json::{json, Map, Value};

use super::matcher::{strip_suffix, tokens, trigram_sim};
use crate::model::{Action, ActionsFile, ButtonItem, LayoutFile, LayoutItem, Page};

/// "The page has no free place" (i18n `voice.reply.no_room`).
pub const NO_ROOM: &str = "No space on the page";

const CATALOG: &str = include_str!("../../../../shared/catalog/catalog.json");
const OVERLAYS: [(&str, &str); 9] = [
    ("zh", include_str!("../../../../shared/catalog/i18n/zh.json")),
    ("hi", include_str!("../../../../shared/catalog/i18n/hi.json")),
    ("es", include_str!("../../../../shared/catalog/i18n/es.json")),
    ("ar", include_str!("../../../../shared/catalog/i18n/ar.json")),
    ("fr", include_str!("../../../../shared/catalog/i18n/fr.json")),
    ("pt", include_str!("../../../../shared/catalog/i18n/pt.json")),
    ("de", include_str!("../../../../shared/catalog/i18n/de.json")),
    ("ja", include_str!("../../../../shared/catalog/i18n/ja.json")),
    ("kk", include_str!("../../../../shared/catalog/i18n/kk.json")),
];

/// A ready-made key in one language.
#[derive(Debug, Clone)]
pub struct Entry {
    pub id: String,
    pub name: String,
    pub label: String,
    /// The English name: the id of the action is made of it, the same in every language.
    english: String,
    /// Every name the key can be asked for by (its language and English).
    spoken: Vec<String>,
    raw: Value,
}

impl Entry {
    /// What is missing and has to be typed: the settings window asks for it.
    pub fn asks(&self) -> bool {
        self.raw.get("ask").is_some()
    }

    /// A plain key with an action: widgets and navigation keys are placed in the editor.
    pub fn is_button(&self) -> bool {
        self.raw.get("widget").is_none() && self.raw.get("navigate").is_none() && (self.asks() || self.raw.get("action").is_some())
    }

    pub fn icon(&self) -> String {
        let icon = self.raw.get("icon").and_then(Value::as_str).unwrap_or("lucide:square");
        // Phosphor pictures are made into images by the settings window; here the line icon of the same name stands in.
        match icon.strip_prefix("ph:") {
            Some(name) => format!("lucide:{name}"),
            None => icon.to_string(),
        }
    }
}

fn text(v: Option<&Value>, lang: &str) -> Option<String> {
    v?.get(lang).and_then(Value::as_str).map(str::to_string)
}

/// The catalog with the texts of `lang` (English where it has none).
pub fn catalog(lang: &str) -> Vec<Entry> {
    let data: Value = serde_json::from_str(CATALOG).expect("the embedded catalog is valid JSON");
    let overlay: Option<Value> = OVERLAYS.iter().find(|(code, _)| *code == lang).map(|(_, t)| serde_json::from_str(t).expect("the embedded overlay is valid JSON"));
    data["items"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|raw| {
            let id = raw.get("id")?.as_str()?.to_string();
            let over = overlay.as_ref().and_then(|o| o.get("items")?.get(&id));
            let in_lang = |field: &str| over.and_then(|o| o.get(field)?.as_str().map(str::to_string)).or_else(|| text(raw.get(field), lang));
            let english = text(raw.get("name"), "en")?;
            let name = in_lang("name").unwrap_or_else(|| english.clone());
            let label = in_lang("label").or_else(|| text(raw.get("label"), "en")).unwrap_or_else(|| name.clone());
            let mut spoken = vec![name.clone(), label.clone(), english.clone()];
            spoken.extend(text(raw.get("label"), "en"));
            Some(Entry { id, name, label, english, spoken, raw: raw.clone() })
        })
        .collect()
}

/// The ids a voice request may name (buttons only), for the schema of the model.
pub fn button_ids() -> Vec<String> {
    catalog("en").into_iter().filter(Entry::is_button).map(|e| e.id).collect()
}

/// A word as it is compared: its beginning stands for it, so that an inflected word finds its name ("блокировки").
fn same_word(a: &str, b: &str) -> bool {
    let (la, lb) = (a.chars().count(), b.chars().count());
    if a == b {
        return true;
    }
    let stem = la.min(lb).saturating_sub(2).max(4);
    la >= 4 && lb >= 4 && a.chars().take(stem).eq(b.chars().take(stem))
}

fn score(spoken: &[String], known: &str) -> f64 {
    let known = tokens(known);
    if known == spoken {
        return 1.0;
    }
    if spoken.is_empty() || known.is_empty() {
        return 0.0;
    }
    let found = spoken.iter().filter(|w| known.iter().any(|k| same_word(w, k))).count();
    if found == spoken.len() {
        // Every spoken word is in the name; a name with fewer other words fits better.
        return 0.7 + 0.2 * found as f64 / known.len().max(found) as f64;
    }
    let s = trigram_sim(&spoken.join(" "), &known.join(" "));
    if s >= 0.6 {
        s * 0.8
    } else {
        0.0
    }
}

/// The buttons that fit a spoken name best: one, or several that fit equally well.
pub fn find<'a>(lang: &str, spoken: &str, entries: &'a [Entry]) -> Vec<&'a Entry> {
    let mut words = tokens(spoken);
    if let Some(stem) = words.last().and_then(|w| strip_suffix(lang, w)) {
        if entries.iter().all(|e| e.spoken.iter().all(|n| score(&words, n) == 0.0)) {
            *words.last_mut().expect("there is a last word") = stem;
        }
    }
    let scored: Vec<(f64, &Entry)> =
        entries.iter().filter(|e| e.is_button()).map(|e| (e.spoken.iter().map(|n| score(&words, n)).fold(0.0, f64::max), e)).filter(|s| s.0 > 0.0).collect();
    let top = scored.iter().map(|s| s.0).fold(0.0, f64::max);
    scored.into_iter().filter(|s| top - s.0 < 0.02).map(|s| s.1).collect()
}

/// The first free cell of a page, reading order.
pub fn free_cell(page: &Page, cols: i32, rows: i32) -> Option<(i32, i32)> {
    (0..rows).flat_map(|y| (0..cols).map(move |x| (x, y))).find(|&(x, y)| {
        page.items.iter().all(|item| {
            let (_, ix, iy, w, h) = item.rect();
            !(x >= ix && x < ix + w && y >= iy && y < iy + h)
        })
    })
}

/// `^[a-z0-9_-]{1,64}$` from a name, not taken yet.
fn new_id(name: &str, taken: &HashSet<String>) -> String {
    let mut slug = String::new();
    for c in name.to_lowercase().chars() {
        if c.is_ascii_alphanumeric() {
            slug.push(c);
        } else if !slug.is_empty() && !slug.ends_with('-') {
            slug.push('-');
        }
    }
    let slug: String = slug.trim_matches('-').chars().take(56).collect();
    let slug = if slug.is_empty() { "button".to_string() } else { slug };
    if !taken.contains(&slug) {
        return slug;
    }
    (2..).map(|n| format!("{slug}-{n}")).find(|c| !taken.contains(c)).expect("an unused suffix exists")
}

/// An action without its name, to find an equal one that exists already.
fn body_of(action: &Action) -> Option<Value> {
    let mut v = serde_json::to_value(action).ok()?;
    v.as_object_mut()?.remove("name");
    Some(v)
}

/// The new files with the button of `catalog_id` on `page_id`. Nothing is written here; the caller saves through the checked
/// save, which refuses the whole file when anything is wrong.
pub fn build(lang: &str, catalog_id: &str, page_id: &str, actions: &ActionsFile, layout: &LayoutFile) -> Result<(ActionsFile, LayoutFile), String> {
    let entries = catalog(lang);
    let entry = entries.iter().find(|e| e.id == catalog_id && e.is_button() && !e.asks()).ok_or("That button cannot be added by voice")?;
    let page = layout.pages.iter().position(|p| p.id == page_id).ok_or("The page is gone")?;
    let (x, y) = free_cell(&layout.pages[page], i32::from(layout.grid.cols), i32::from(layout.grid.rows)).ok_or_else(|| NO_ROOM.to_string())?;

    let mut body: Map<String, Value> = entry.raw.get("action").and_then(Value::as_object).cloned().ok_or("That button cannot be added by voice")?;
    body.insert("name".into(), Value::String(entry.name.chars().take(60).collect()));
    let action: Action = serde_json::from_value(Value::Object(body)).map_err(|e| format!("The catalog key is invalid: {e}"))?;
    let mut new_actions = actions.clone();
    let wanted = body_of(&action);
    let action_id = match new_actions.actions.iter().find(|(_, a)| body_of(a) == wanted) {
        Some((id, _)) => id.clone(),
        None => {
            let id = new_id(&entry.english, &new_actions.actions.keys().cloned().collect());
            new_actions.actions.insert(id.clone(), action);
            id
        }
    };

    let items: HashSet<String> = layout.pages.iter().flat_map(|p| p.items.iter().map(|i| i.rect().0.to_string())).collect();
    let accent = layout.display.accent.clone().unwrap_or_else(|| "#7c5cff".to_string());
    let colour = |c: &str| if c == "$accent" { accent.clone() } else { c.to_string() };
    let mut button = json!({
        "id": new_id(&format!("v-{}", entry.id), &items),
        "x": x, "y": y, "w": 1, "h": 1,
        "label": entry.label.chars().take(12).collect::<String>(),
        "icon": entry.icon(),
        "actionId": action_id,
        "catalogId": entry.id,
    });
    let o = button.as_object_mut().expect("an object");
    if let Some(c) = entry.raw.get("color").and_then(Value::as_str) {
        o.insert("color".into(), Value::String(colour(c)));
    }
    if entry.raw.get("confirm").and_then(Value::as_bool) == Some(true) {
        o.insert("confirm".into(), Value::Bool(true));
    }
    if let Some(bind) = entry.raw.get("bind") {
        let states: BTreeMap<String, Value> = bind["states"]
            .as_object()
            .into_iter()
            .flatten()
            .map(|(state, look)| {
                let mut out = Map::new();
                if let Some(l) = text(look.get("label"), lang).or_else(|| text(look.get("label"), "en")) {
                    out.insert("label".into(), Value::String(l.chars().take(12).collect()));
                }
                if let Some(i) = look.get("icon").and_then(Value::as_str) {
                    out.insert("icon".into(), Value::String(i.to_string()));
                }
                if let Some(c) = look.get("color").and_then(Value::as_str) {
                    out.insert("color".into(), Value::String(colour(c)));
                }
                (state.clone(), Value::Object(out))
            })
            .collect();
        o.insert("bind".into(), json!({ "var": bind["var"], "states": states }));
    }
    let button: ButtonItem = serde_json::from_value(button).map_err(|e| format!("The catalog key is invalid: {e}"))?;
    let mut new_layout = layout.clone();
    new_layout.pages[page].items.push(LayoutItem::Button(button));
    Ok((new_actions, new_layout))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{default_actions, default_layout};
    use crate::validate::{validate_actions, validate_layout};

    fn best(lang: &str, said: &str) -> Vec<String> {
        let entries = catalog(lang);
        find(lang, said, &entries).iter().map(|e| e.id.clone()).collect()
    }

    #[test]
    fn a_button_is_found_by_its_name_in_the_language_or_in_english() {
        assert_eq!(best("ru", "блокировки"), ["win.lock"]);
        assert_eq!(best("ru", "Блокировка ПК"), ["win.lock"]);
        assert_eq!(best("en", "lock"), ["win.lock"]);
        assert_eq!(best("de", "Lock PC"), ["win.lock"]);
        assert!(best("en", "format my disk").is_empty(), "nothing outside the catalog");
        assert!(best("en", "clock widget").is_empty(), "widgets are placed in the editor");
    }

    #[test]
    fn every_plain_key_of_the_catalog_makes_files_that_pass_the_check() {
        let layout = {
            let mut l = default_layout();
            l.pages[0].items.clear();
            l
        };
        let actions = default_actions();
        let page = layout.pages[0].id.clone();
        for e in catalog("ru").iter().filter(|e| e.is_button() && !e.asks()) {
            let (a, l) = build("ru", &e.id, &page, &actions, &layout).unwrap_or_else(|err| panic!("{}: {err}", e.id));
            assert!(validate_actions(&a).is_empty(), "{}: {:?}", e.id, validate_actions(&a));
            let issues: Vec<_> = validate_layout(&l, &a).into_iter().filter(|i| !i.path.starts_with("/pages/1")).collect();
            assert!(issues.is_empty(), "{}: {issues:?}", e.id);
        }
    }

    #[test]
    fn the_button_goes_into_the_first_free_cell_and_a_full_page_says_so() {
        let mut layout = default_layout();
        layout.pages[0].items.clear();
        let actions = default_actions();
        let page = layout.pages[0].id.clone();
        let (a, l) = build("en", "win.lock", &page, &actions, &layout).unwrap();
        let LayoutItem::Button(b) = &l.pages[0].items[0] else { panic!() };
        assert_eq!((b.x, b.y, b.label.as_deref(), b.confirm), (0, 0, Some("Lock"), Some(true)));
        assert_eq!(b.catalog_id.as_deref(), Some("win.lock"));
        // The same action is reused by the next button, which takes the next cell.
        let (a2, l2) = build("en", "win.lock", &page, &a, &l).unwrap();
        assert_eq!(a2.actions.len(), a.actions.len());
        let LayoutItem::Button(second) = &l2.pages[0].items[1] else { panic!() };
        assert_eq!((second.x, second.y), (1, 0));
        assert_ne!(second.id, b.id);
        // A full page.
        let mut full = layout.clone();
        for y in 0..i32::from(full.grid.rows) {
            for x in 0..i32::from(full.grid.cols) {
                full.pages[0].items.push(serde_json::from_value(json!({ "kind": "clock", "id": format!("c{x}-{y}"), "x": x, "y": y, "w": 1, "h": 1 })).unwrap());
            }
        }
        assert_eq!(build("en", "win.lock", &page, &actions, &full).unwrap_err(), NO_ROOM);
    }

    #[test]
    fn only_plain_keys_of_the_catalog_can_be_built() {
        let (layout, actions) = (default_layout(), default_actions());
        let page = layout.pages[0].id.clone();
        for id in ["web.site", "keys.shortcut", "widget.clock", "nope", "nav.back"] {
            assert!(build("en", id, &page, &actions, &layout).is_err(), "{id}");
        }
    }
}
