//! The grid size is configurable (DATA_MODEL.md 3.1, 3.2): every bound check follows `grid.cols`/`grid.rows`.

use decka_core::config::{default_actions, default_layout};
use decka_core::model::LayoutFile;
use decka_core::validate::validate_layout;
use serde_json::{json, Value};

fn layout(cols: u64, rows: u64, items: Value) -> Result<LayoutFile, serde_json::Error> {
    let base = serde_json::to_value(default_layout()).unwrap();
    serde_json::from_value(json!({
        "version": 1,
        "grid": { "cols": cols, "rows": rows },
        "display": base["display"],
        "pages": [{ "id": "main", "name": "Main", "items": items }],
    }))
}

fn paths(cols: u64, rows: u64, items: Value) -> Vec<String> {
    validate_layout(&layout(cols, rows, items).unwrap(), &default_actions()).into_iter().map(|i| i.path).collect()
}

fn button(x: i64, y: i64) -> Value {
    json!([{ "id": "b", "kind": "button", "x": x, "y": y, "w": 1, "h": 1, "actionId": "lock" }])
}

#[test]
fn the_default_layout_is_7x4_and_valid() {
    let default = default_layout();
    assert_eq!((default.grid.cols, default.grid.rows), (7, 4));
    assert!(validate_layout(&default, &default_actions()).is_empty());
}

#[test]
fn eight_by_five_is_valid_and_nine_by_four_or_seven_by_one_are_errors() {
    assert!(paths(8, 5, json!([])).is_empty());
    for (c, r) in [(9, 4), (7, 1), (2, 4), (7, 6)] {
        assert_eq!(paths(c, r, json!([])), ["/grid"], "{c}x{r}");
    }
}

fn messages_in(orientation: Option<&str>, cols: u64, rows: u64) -> Vec<String> {
    let mut l = layout(cols, rows, json!([])).unwrap();
    l.display.orientation = orientation.map(str::to_string);
    validate_layout(&l, &default_actions()).into_iter().map(|i| i.message).collect()
}

#[test]
fn the_limits_follow_the_orientation_and_both_messages_say_which() {
    // Landscape (also when `orientation` is absent): 3..8 columns, 2..5 rows.
    assert_eq!(messages_in(None, 9, 4), ["Grid 9×4 does not fit landscape (3–8 columns, 2–5 rows)"]);
    assert_eq!(messages_in(Some("landscape"), 4, 7), ["Grid 4×7 does not fit landscape (3–8 columns, 2–5 rows)"]);
    // Portrait: 2..5 columns, 3..8 rows.
    assert_eq!(messages_in(Some("portrait"), 7, 4), ["Grid 7×4 does not fit portrait (2–5 columns, 3–8 rows)"]);
    assert_eq!(messages_in(Some("portrait"), 4, 2), ["Grid 4×2 does not fit portrait (2–5 columns, 3–8 rows)"]);
    for (c, r) in [(2, 3), (4, 7), (5, 8)] {
        assert!(messages_in(Some("portrait"), c, r).is_empty(), "{c}x{r}");
    }
    for (c, r) in [(3, 2), (7, 4), (8, 5)] {
        assert!(messages_in(None, c, r).is_empty(), "{c}x{r}");
    }
}

#[test]
fn orientation_must_be_landscape_or_portrait() {
    let mut l = layout(4, 7, json!([])).unwrap();
    l.display.orientation = Some("diagonal".into());
    let p: Vec<String> = validate_layout(&l, &default_actions()).into_iter().map(|i| i.path).collect();
    assert!(p.contains(&"/display/orientation".to_string()), "{p:?}");
}

#[test]
fn an_item_at_x_6_fits_seven_columns_but_not_six() {
    assert!(paths(7, 4, button(6, 0)).is_empty());
    assert_eq!(paths(6, 4, button(6, 0)), ["/pages/0/items/0"]);
}

#[test]
fn list_widgets_may_fill_the_whole_grid() {
    let widget = json!([{ "id": "n", "kind": "notifications", "x": 0, "y": 0, "w": 8, "h": 5 }]);
    assert!(paths(8, 5, widget.clone()).is_empty());
    assert_eq!(paths(7, 5, widget), ["/pages/0/items/0", "/pages/0/items/0"]); // outside the grid, and too wide
}

fn with_pages(pages: Value) -> Vec<String> {
    let base = serde_json::to_value(default_layout()).unwrap();
    let layout: LayoutFile = serde_json::from_value(json!({ "version": 1, "grid": base["grid"], "display": base["display"], "pages": pages })).unwrap();
    validate_layout(&layout, &default_actions()).into_iter().map(|i| i.path).collect()
}

#[test]
fn context_page_apps_follow_the_m11_rules() {
    let page = |id: &str, apps: Value| json!({ "id": id, "name": id, "items": [], "apps": apps });
    // One app on two pages: the error is on the second page.
    assert_eq!(with_pages(json!([page("a", json!([])), page("b", json!(["code.exe"])), page("c", json!(["code.exe"]))])), ["/pages/2/apps/0"]);
    // The first page is home and cannot have apps.
    assert_eq!(with_pages(json!([page("a", json!(["code.exe"])), page("b", json!([]))])), ["/pages/0/apps"]);
    // Names must look like an exe.
    assert_eq!(with_pages(json!([page("a", json!([])), page("b", json!(["notepad"]))])), ["/pages/1/apps/0"]);
}

#[test]
fn the_media_widget_has_size_and_action_rules() {
    let media = |extra: Value| {
        let mut item = json!({ "id": "m", "kind": "media", "x": 0, "y": 0, "w": 3, "h": 1 });
        item.as_object_mut().unwrap().extend(extra.as_object().unwrap().clone());
        json!([{ "id": "p", "name": "P", "items": [item] }])
    };
    assert!(with_pages(media(json!({}))).is_empty());
    assert!(with_pages(media(json!({ "playAction": "media-play" }))).is_empty());
    assert_eq!(with_pages(media(json!({ "w": 1 }))), ["/pages/0/items/0"]); // 1x1
    assert_eq!(with_pages(media(json!({ "h": 3 }))), ["/pages/0/items/0"]); // 3x3
    assert_eq!(with_pages(media(json!({ "playAction": "nope" }))), ["/pages/0/items/0"]);
}

#[test]
fn forty_items_per_page_are_allowed_forty_one_are_not() {
    let many = |n: usize| -> Value {
        Value::Array((0..n).map(|i| json!({ "id": format!("c{i}"), "kind": "clock", "x": 0, "y": 0, "w": 1, "h": 1 })).collect())
    };
    // Overlaps are reported too, so only look at the per-page limit message.
    let count = |n: usize| {
        validate_layout(&layout(8, 5, many(n)).unwrap(), &default_actions()).into_iter().filter(|i| i.message.contains("items per page")).count()
    };
    assert_eq!(count(40), 0);
    assert_eq!(count(41), 1);
}

#[test]
fn key_face_must_be_standard_or_full() {
    let item = |face: &str| json!([{ "id": "b", "kind": "button", "x": 0, "y": 0, "w": 1, "h": 1, "actionId": "lock", "face": face, "showLabel": false }]);
    assert!(paths(5, 3, item("full")).is_empty());
    assert_eq!(paths(5, 3, item("standard")), Vec::<String>::new());
    assert_eq!(paths(5, 3, item("huge")), ["/pages/0/items/0/face"]);

    let mut l = layout(5, 3, button(0, 0)).unwrap();
    l.display.key_face = Some("huge".into());
    let p: Vec<String> = validate_layout(&l, &default_actions()).into_iter().map(|i| i.path).collect();
    assert_eq!(p, ["/display/keyFace"]);
    l.display.key_face = Some("full".into());
    l.display.key_labels = Some(false);
    assert!(validate_layout(&l, &default_actions()).is_empty());
}

#[test]
fn language_must_be_auto_or_one_of_the_eleven() {
    let mut l = layout(5, 3, button(0, 0)).unwrap();
    for ok in ["auto", "en", "zh", "hi", "es", "ar", "fr", "pt", "ru", "de", "ja", "kk"] {
        l.display.language = Some(ok.into());
        assert!(validate_layout(&l, &default_actions()).is_empty(), "{ok}");
    }
    l.display.language = Some("xx".into());
    let p: Vec<String> = validate_layout(&l, &default_actions()).into_iter().map(|i| i.path).collect();
    assert_eq!(p, ["/display/language"]);
}
