//! Page templates (DATA_MODEL.md 7): ready-made context pages, embedded from `shared/templates/*.json`.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::model::{Action, ActionsFile, Grid, LayoutFile, LayoutItem, Page, ValidationIssue};
use crate::validate::{validate_actions, validate_layout};

const SOURCES: [&str; 13] = [
    include_str!("../../../shared/templates/outlook.json"),
    include_str!("../../../shared/templates/word.json"),
    include_str!("../../../shared/templates/excel.json"),
    include_str!("../../../shared/templates/powerpoint.json"),
    include_str!("../../../shared/templates/canva.json"),
    include_str!("../../../shared/templates/telegram.json"),
    include_str!("../../../shared/templates/whatsapp.json"),
    include_str!("../../../shared/templates/discord.json"),
    include_str!("../../../shared/templates/teams.json"),
    include_str!("../../../shared/templates/vscode.json"),
    include_str!("../../../shared/templates/browser.json"),
    include_str!("../../../shared/templates/zoom.json"),
    include_str!("../../../shared/templates/spotify.json"),
];

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MinGrid {
    pub cols: u8,
    pub rows: u8,
}

/// Mirrors `PageTemplate` in shared/src/templates.ts.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PageTemplate {
    pub id: String,
    pub name: String,
    pub apps: Vec<String>,
    pub min_grid: MinGrid,
    pub actions: BTreeMap<String, Action>,
    pub items: Vec<LayoutItem>,
    /// The name of the template in the other languages by code (M19: the Russian one is `nameRu` of the file, the others come
    /// from `shared/catalog/i18n/<code>.json`); `name` is English and the fallback.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub names: BTreeMap<String, String>,
    /// The names of the actions by language code and action id (DATA_MODEL.md 10.5): the captions of the app strip.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub action_names: BTreeMap<String, BTreeMap<String, String>>,
}

/// The texts of the templates in the languages other than English and Russian (`templates.name`, `templates.actions` of each file).
const OVERLAYS: [(&str, &str); 9] = [
    ("zh", include_str!("../../../shared/catalog/i18n/zh.json")),
    ("hi", include_str!("../../../shared/catalog/i18n/hi.json")),
    ("es", include_str!("../../../shared/catalog/i18n/es.json")),
    ("ar", include_str!("../../../shared/catalog/i18n/ar.json")),
    ("fr", include_str!("../../../shared/catalog/i18n/fr.json")),
    ("pt", include_str!("../../../shared/catalog/i18n/pt.json")),
    ("de", include_str!("../../../shared/catalog/i18n/de.json")),
    ("ja", include_str!("../../../shared/catalog/i18n/ja.json")),
    ("kk", include_str!("../../../shared/catalog/i18n/kk.json")),
];

/// The built-in templates.
pub fn all() -> Vec<PageTemplate> {
    let overlays: Vec<(&str, serde_json::Value)> = OVERLAYS.iter().map(|(code, text)| (*code, serde_json::from_str(text).expect("embedded overlay is valid JSON"))).collect();
    SOURCES
        .iter()
        .map(|text| {
            let mut template: PageTemplate = serde_json::from_str(text).expect("embedded template is valid");
            let raw: serde_json::Value = serde_json::from_str(text).expect("embedded template is valid JSON");
            if let Some(name) = raw.get("nameRu").and_then(|v| v.as_str()) {
                template.names.insert("ru".to_string(), name.to_string());
            }
            if let Some(actions) = raw.get("actions").and_then(|a| a.as_object()) {
                let russian: BTreeMap<String, String> = actions.iter().filter_map(|(id, a)| Some((id.clone(), a.get("nameRu")?.as_str()?.to_string()))).collect();
                if !russian.is_empty() {
                    template.action_names.insert("ru".to_string(), russian);
                }
            }
            for (code, overlay) in &overlays {
                let Some(templates) = overlay.get("templates") else { continue };
                if let Some(name) = templates.get("name").and_then(|n| n.get(&template.id)).and_then(|v| v.as_str()) {
                    template.names.insert(code.to_string(), name.to_string());
                }
                if let Some(map) = templates.get("actions").and_then(|a| a.get(&template.id)).and_then(|v| v.as_object()) {
                    let names: BTreeMap<String, String> = map.iter().filter_map(|(id, v)| Some((id.clone(), v.as_str()?.to_string()))).collect();
                    template.action_names.insert(code.to_string(), names);
                }
            }
            template
        })
        .collect()
}

fn issue(path: impl Into<String>, message: impl Into<String>) -> ValidationIssue {
    ValidationIssue { path: path.into(), message: message.into() }
}

/// First page id of the form `<id>`, `<id>-2`, `<id>-3`, ... that is not taken.
fn free_page_id(layout: &LayoutFile, id: &str) -> String {
    let taken = |candidate: &str| layout.pages.iter().any(|p| p.id == candidate);
    if !taken(id) {
        return id.to_string();
    }
    (2..).map(|n| format!("{id}-{n}")).find(|c| !taken(c)).expect("an unused suffix exists")
}

/// The items of a landscape page turned for a `cols` x `rows` portrait grid like `rotateLayout` does it (DATA_MODEL.md 3.7):
/// widgets first, then buttons, each group by (y, x); every item goes to the first free spot of its size, top to bottom, left
/// to right (reading order is kept). Sizes do not change. `None` when something has no place.
fn turn_items(items: Vec<LayoutItem>, cols: i32, rows: i32) -> Option<Vec<LayoutItem>> {
    let mut order = items;
    order.sort_by_key(|item| {
        let (_, x, y, _, _) = item.rect();
        (item.kind_name() == "button", y, x)
    });
    let mut placed: Vec<LayoutItem> = Vec::new();
    let free = |placed: &[LayoutItem], x: i32, y: i32, w: i32, h: i32| {
        x >= 0 && y >= 0 && x + w <= cols && y + h <= rows && placed.iter().all(|o| {
            let (_, ox, oy, ow, oh) = o.rect();
            !(x < ox + ow && ox < x + w && y < oy + oh && oy < y + h)
        })
    };
    for mut item in order {
        let (_, _, _, w, h) = item.rect();
        let spot = (0..rows).flat_map(|yy| (0..cols).map(move |xx| (xx, yy))).find(|&(xx, yy)| free(&placed, xx, yy, w, h))?;
        item.set_pos(spot.0, spot.1);
        placed.push(item);
    }
    Some(placed)
}

/// The algorithm of DATA_MODEL.md 7 without any I/O: returns the new actions and layout, or the issues.
/// Nothing is changed in `actions` / `layout`; the caller saves the result.
pub fn build(template_id: &str, layout: &LayoutFile, actions: &ActionsFile) -> Result<(ActionsFile, LayoutFile), Vec<ValidationIssue>> {
    let Some(template) = all().into_iter().find(|t| t.id == template_id) else {
        return Err(vec![issue("/templates", "unknown template")]);
    };
    let Grid { cols, rows } = layout.grid;
    // The templates are drawn for a landscape deck; on a portrait one `minGrid` is turned like the page (DATA_MODEL.md 3.7).
    let portrait = layout.display.orientation.as_deref() == Some("portrait");
    let (need_cols, need_rows) = if portrait { (template.min_grid.rows, template.min_grid.cols) } else { (template.min_grid.cols, template.min_grid.rows) };
    if cols < need_cols || rows < need_rows {
        return Err(vec![issue("/grid", format!("Template needs a {need_cols}×{need_rows} grid or larger"))]);
    }
    for (pi, page) in layout.pages.iter().enumerate() {
        if let Some(app) = template.apps.iter().find(|a| page.apps.as_ref().is_some_and(|used| used.contains(a))) {
            return Err(vec![issue(format!("/pages/{pi}/apps"), format!("{app} is already used by page {}", page.name))]);
        }
    }
    if layout.pages.len() >= 10 {
        return Err(vec![issue("/pages", "Page limit reached")]);
    }

    let page_id = free_page_id(layout, &template.id);
    let home = layout.pages.first().map(|p| p.id.clone()).unwrap_or_default();

    let mut new_actions = actions.clone();
    for (id, action) in &template.actions {
        new_actions.actions.entry(id.clone()).or_insert_with(|| action.clone());
    }

    let items = template
        .items
        .iter()
        .cloned()
        .map(|mut item| {
            match &mut item {
                LayoutItem::Button(b) => {
                    b.id = format!("t-{page_id}-{}", b.id);
                    if b.navigate.as_deref() == Some("$home") {
                        b.navigate = Some(home.clone());
                    }
                }
                LayoutItem::Clock(i) => i.id = format!("t-{page_id}-{}", i.id),
                LayoutItem::Sys(i) => i.id = format!("t-{page_id}-{}", i.id),
                LayoutItem::Statuses(i) => i.id = format!("t-{page_id}-{}", i.id),
                LayoutItem::Status(i) => i.id = format!("t-{page_id}-{}", i.id),
                LayoutItem::Notifications(i) => i.id = format!("t-{page_id}-{}", i.id),
                LayoutItem::Media(i) => i.id = format!("t-{page_id}-{}", i.id),
                LayoutItem::Volume(i) => i.id = format!("t-{page_id}-{}", i.id),
                LayoutItem::Timer(i) => i.id = format!("t-{page_id}-{}", i.id),
                LayoutItem::Weather(i) | LayoutItem::Obs(i) | LayoutItem::Twitch(i) | LayoutItem::Youtube(i) | LayoutItem::Context(i) | LayoutItem::Battery(i) | LayoutItem::Mail(i) => i.id = format!("t-{page_id}-{}", i.id),
                LayoutItem::Ai(i) => i.id = format!("t-{page_id}-{}", i.id),
            }
            item
        })
        .collect();

    let items: Vec<LayoutItem> = if portrait {
        match turn_items(items, cols as i32, rows as i32) {
            Some(turned) => turned,
            None => return Err(vec![issue("/grid", format!("Template needs a {need_cols}×{need_rows} grid or larger"))]),
        }
    } else {
        items
    };

    let mut new_layout = layout.clone();
    new_layout.pages.push(Page { id: page_id, name: template.name.clone(), items, apps: Some(template.apps.clone()), popup: None, stay_open: None, time: None });

    let mut issues = validate_actions(&new_actions);
    issues.extend(validate_layout(&new_layout, &new_actions));
    if issues.is_empty() {
        Ok((new_actions, new_layout))
    } else {
        Err(issues)
    }
}
