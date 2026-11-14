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
    assert_eq!(paths(9, 4, json!([])), ["/grid/cols"]);
    assert_eq!(paths(7, 1, json!([])), ["/grid/rows"]);
    assert_eq!(paths(2, 4, json!([])), ["/grid/cols"]);
    assert_eq!(paths(7, 6, json!([])), ["/grid/rows"]);
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
