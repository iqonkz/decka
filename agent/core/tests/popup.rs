//! Rule 12 (popup groups) in the Rust validator: same paths and texts as shared/src/validate.ts.

use decka_core::config::default_actions;
use decka_core::model::{LayoutFile, LayoutItem};
use decka_core::validate::validate_layout;

const FIXTURE: &str = include_str!("../../../shared/fixtures/layout.popup.json");

fn fixture() -> LayoutFile {
    serde_json::from_str(FIXTURE).expect("layout.popup.json parses")
}

fn problems(mutate: impl FnOnce(&mut LayoutFile)) -> Vec<(String, String)> {
    let mut layout = fixture();
    mutate(&mut layout);
    validate_layout(&layout, &default_actions()).into_iter().map(|i| (i.path, i.message)).collect()
}

fn button<'a>(layout: &'a mut LayoutFile, id: &str) -> &'a mut decka_core::model::ButtonItem {
    layout
        .pages
        .iter_mut()
        .flat_map(|p| p.items.iter_mut())
        .find_map(|i| match i {
            LayoutItem::Button(b) if b.id == id => Some(b),
            _ => None,
        })
        .expect("button exists")
}

fn one(path: &str, message: &str) -> Vec<(String, String)> {
    vec![(path.to_string(), message.to_string())]
}

#[test]
fn the_fixture_is_valid_and_an_action_may_open_a_group() {
    assert!(problems(|_| {}).is_empty());
    assert!(problems(|l| button(l, "b-more").action_id = Some("media-play".into())).is_empty());
}

#[test]
fn nothing_to_do_and_conflicting_targets_are_refused() {
    assert_eq!(problems(|l| button(l, "b-more").popup = None), one("/pages/0/items/14", "choose an action or a page for this button"));
    assert_eq!(problems(|l| button(l, "b-more").navigate = Some("dev".into())), one("/pages/0/items/14", "a button can open a page or a popup group, not both"));
    assert_eq!(problems(|l| button(l, "b-to-dev").action_id = Some("media-play".into())), one("/pages/0/items/15", "a button can either run an action or open a page, not both"));
}

#[test]
fn targets_must_be_the_right_kind_of_page() {
    assert_eq!(problems(|l| button(l, "b-more").popup = Some("nowhere".into())), one("/pages/0/items/14", "popup group \"nowhere\" no longer exists — choose another"));
    assert_eq!(problems(|l| button(l, "b-more").popup = Some("dev".into())), one("/pages/0/items/14", "page \"dev\" is not a popup group"));
    assert_eq!(problems(|l| button(l, "b-to-dev").navigate = Some("media-more".into())), one("/pages/0/items/15", "page \"media-more\" is a popup group — choose a normal page"));
}

#[test]
fn popup_page_restrictions() {
    assert!(problems(|l| l.pages[0].popup = Some(true)).iter().any(|(p, _)| p == "/pages/0/popup"));
    assert_eq!(problems(|l| l.pages[2].apps = Some(vec!["code.exe".into()])), one("/pages/2/apps", "a popup group cannot show itself for apps: remove them"));
    assert_eq!(
        problems(|l| {
            let b = button(l, "b-pop-up");
            b.action_id = None;
            b.navigate = Some("main".into());
        }),
        one("/pages/2/items/2", "a key inside a popup group cannot open a page or another group")
    );
}

#[test]
fn stay_open_belongs_to_popup_pages_only() {
    assert!(problems(|l| l.pages[2].stay_open = Some(true)).is_empty());
    assert_eq!(problems(|l| l.pages[1].stay_open = Some(true)), one("/pages/1/stayOpen", "stayOpen only makes sense for a popup group"));
}
