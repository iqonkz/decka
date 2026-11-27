//! Which collectors have to run (DATA_MODEL.md 10.1): a collector whose section no layout shows and whose variables no
//! `bind` or `toggleVar` reads is not started at all.

use crate::model::{Action, ActionsFile, LayoutFile, LayoutItem};

/// The collectors that can be switched off by the layout. `focus` and `ai` are not here: the active-app collector
/// drives the context pages and the strip, and `ai` also carries the permission questions of Claude Code.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Needed {
    pub sys: bool,
    pub media: bool,
    pub audio: bool,
    pub weather: bool,
    pub obs: bool,
    pub battery: bool,
    pub mail: bool,
}

impl Needed {
    fn mark_var(&mut self, var: &str) {
        if var.starts_with("audio.") || var.starts_with("mic.") {
            self.audio = true;
        } else if var.starts_with("media.") {
            self.media = true;
        } else if var.starts_with("obs.") {
            self.obs = true;
        } else if var.starts_with("battery.") {
            self.battery = true;
        } else if var.starts_with("mail.") {
            self.mail = true;
        }
    }
}

/// Pure: the set of collectors the layout and the actions need.
pub fn needed_collectors(layout: &LayoutFile, actions: &ActionsFile) -> Needed {
    let mut n = Needed::default();
    for page in &layout.pages {
        for item in &page.items {
            match item {
                LayoutItem::Sys(_) => n.sys = true,
                LayoutItem::Media(_) => n.media = true,
                LayoutItem::Volume(_) => n.audio = true,
                LayoutItem::Weather(_) => n.weather = true,
                LayoutItem::Obs(_) => n.obs = true,
                LayoutItem::Battery(_) => n.battery = true,
                LayoutItem::Mail(_) => n.mail = true,
                LayoutItem::Button(b) => {
                    if let Some(bind) = &b.bind {
                        n.mark_var(&bind.var);
                    }
                }
                _ => {}
            }
        }
    }
    for action in actions.actions.values() {
        if let Some(var) = action.toggle_var() {
            n.mark_var(var);
        }
        // An OBS action needs the connection to OBS even when no widget shows it.
        if matches!(action, Action::Obs { .. }) {
            n.obs = true;
        }
    }
    n
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{default_actions, default_layout};
    use crate::model::{Bind, ButtonItem, PlainItem};
    use std::collections::BTreeMap;

    fn plain(id: &str) -> PlainItem {
        PlainItem { id: id.into(), x: 0, y: 0, w: 1, h: 1 }
    }

    fn button_with_bind(var: &str) -> LayoutItem {
        LayoutItem::Button(ButtonItem {
            id: "b".into(),
            x: 0,
            y: 0,
            w: 1,
            h: 1,
            label: None,
            icon: None,
            color: None,
            action_id: None,
            navigate: None,
            popup: None,
            confirm: None,
            bind: Some(Bind { var: var.into(), states: BTreeMap::new() }),
            catalog_id: None,
            face: None,
            show_label: None,
        })
    }

    #[test]
    fn plain_layout_needs_nothing_but_what_it_shows() {
        let mut layout = default_layout();
        layout.pages[0].items.retain(|i| matches!(i, LayoutItem::Clock(_)));
        let actions = ActionsFile { version: 1, actions: BTreeMap::new() };
        assert_eq!(needed_collectors(&layout, &actions), Needed::default());
    }

    #[test]
    fn widgets_switch_their_collectors_on() {
        let mut layout = default_layout();
        layout.pages[0].items = vec![LayoutItem::Weather(plain("w")), LayoutItem::Obs(plain("o"))];
        let n = needed_collectors(&layout, &default_actions());
        assert!(n.weather && n.obs && !n.media && !n.audio && !n.battery && !n.mail);
    }

    #[test]
    fn bound_variables_switch_collectors_on() {
        let mut layout = default_layout();
        layout.pages[0].items = vec![button_with_bind("mic.muted"), button_with_bind("mail.unread"), button_with_bind("battery.low"), button_with_bind("custom.x")];
        let n = needed_collectors(&layout, &ActionsFile { version: 1, actions: BTreeMap::new() });
        assert!(n.audio && n.mail && n.battery && !n.media && !n.obs);
    }
}
