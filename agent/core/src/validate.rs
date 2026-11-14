//! Manual validation of actions, layout and config (DATA_MODEL.md 2.2, 2.3, 3.2, 3.3, 4).
//! No JSON Schema in Rust: every rule is coded by hand. `path` is a JSON pointer.

use std::collections::HashSet;

use crate::model::*;

fn issue(path: impl Into<String>, message: impl Into<String>) -> ValidationIssue {
    ValidationIssue { path: path.into(), message: message.into() }
}

/// Escape a key for use as a JSON pointer segment.
fn esc(s: &str) -> String {
    s.replace('~', "~0").replace('/', "~1")
}

fn chars(s: &str) -> usize {
    s.chars().count()
}

pub fn is_id(s: &str) -> bool {
    (1..=64).contains(&s.len()) && s.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_' || b == b'-')
}

pub fn is_var_name(s: &str) -> bool {
    (1..=64).contains(&s.len())
        && s.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_' || b == b'.' || b == b'-')
}

/// `^[a-z0-9._ ()-]{1,60}\.exe$`
pub fn is_app_name(s: &str) -> bool {
    let Some(stem) = s.strip_suffix(".exe") else { return false };
    (1..=60).contains(&stem.len())
        && stem.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || matches!(b, b'.' | b'_' | b' ' | b'(' | b')' | b'-'))
}

fn is_color(s: &str) -> bool {
    s.len() == 7 && s.starts_with('#') && s[1..].bytes().all(|b| b.is_ascii_hexdigit())
}

fn is_hhmm(s: &str) -> bool {
    let b = s.as_bytes();
    if b.len() != 5 || b[2] != b':' || !b.iter().enumerate().all(|(i, c)| i == 2 || c.is_ascii_digit()) {
        return false;
    }
    let h = (b[0] - b'0') * 10 + (b[1] - b'0');
    let m = (b[3] - b'0') * 10 + (b[4] - b'0');
    h <= 23 && m <= 59
}

pub fn is_key_name(k: &str) -> bool {
    if matches!(k, "ctrl" | "alt" | "shift" | "win")
        || matches!(
            k,
            "enter" | "esc" | "space" | "tab" | "backspace" | "delete" | "home" | "end" | "pageup" | "pagedown" | "up" | "down" | "left" | "right"
        )
    {
        return true;
    }
    let b = k.as_bytes();
    if b.len() == 1 && (b[0].is_ascii_lowercase() || b[0].is_ascii_digit()) {
        return true;
    }
    if b.len() >= 2 && b[0] == b'f' && b[1..].iter().all(|c| c.is_ascii_digit()) && b[1] != b'0' {
        return k[1..].parse::<u32>().map(|n| (1..=24).contains(&n)).unwrap_or(false);
    }
    false
}

fn is_http_url(s: &str) -> bool {
    chars(s) <= 2000 && url::Url::parse(s).map(|u| matches!(u.scheme(), "http" | "https")).unwrap_or(false)
}

fn is_valid_icon(s: &str) -> bool {
    if let Some(name) = s.strip_prefix("lucide:") {
        return (1..=40).contains(&name.len()) && name.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-');
    }
    if let Some(sym) = s.strip_prefix("emoji:") {
        return (1..=8).contains(&sym.encode_utf16().count());
    }
    if let Some(hash) = s.strip_prefix("img:") {
        return crate::icons::is_hash(hash);
    }
    false
}

// ===== actions =====

pub fn validate_actions(file: &ActionsFile) -> Vec<ValidationIssue> {
    let mut out = Vec::new();
    if file.version != 1 {
        out.push(issue("/version", "version must be 1"));
    }
    if file.actions.len() > 200 {
        out.push(issue("/actions", "at most 200 actions"));
    }
    for (id, action) in &file.actions {
        let base = format!("/actions/{}", esc(id));
        if !is_id(id) {
            out.push(issue(base.clone(), format!("invalid action id '{id}'")));
        }
        if !(1..=60).contains(&chars(action.name())) {
            out.push(issue(format!("{base}/name"), "name must be 1..60 characters"));
        }
        if let Some(v) = action.toggle_var() {
            if !is_var_name(v) {
                out.push(issue(format!("{base}/toggleVar"), "invalid variable name"));
            }
        }
        match action {
            Action::Hotkey { keys, .. } => {
                if !(1..=5).contains(&keys.len()) {
                    out.push(issue(format!("{base}/keys"), "keys must contain 1..5 entries"));
                }
                let mut seen = HashSet::new();
                for (i, k) in keys.iter().enumerate() {
                    if !is_key_name(k) {
                        out.push(issue(format!("{base}/keys/{i}"), format!("unknown key '{k}'")));
                    } else if !seen.insert(k.as_str()) {
                        out.push(issue(format!("{base}/keys/{i}"), format!("duplicate key '{k}'")));
                    }
                }
            }
            Action::Media { .. } | Action::System { .. } => {}
            Action::Launch { program, args, cwd, .. } => {
                if !(1..=500).contains(&chars(program)) {
                    out.push(issue(format!("{base}/program"), "program must be 1..500 characters"));
                }
                if let Some(args) = args {
                    if args.len() > 32 {
                        out.push(issue(format!("{base}/args"), "at most 32 arguments"));
                    }
                    for (i, a) in args.iter().enumerate() {
                        if chars(a) > 500 {
                            out.push(issue(format!("{base}/args/{i}"), "argument longer than 500 characters"));
                        }
                    }
                }
                if cwd.as_deref().map(chars).unwrap_or(0) > 500 {
                    out.push(issue(format!("{base}/cwd"), "cwd longer than 500 characters"));
                }
            }
            Action::OpenUrl { url, .. } => {
                if !is_http_url(url) {
                    out.push(issue(format!("{base}/url"), "url must be http or https, up to 2000 characters"));
                }
            }
        }
    }
    out
}

// ===== layout =====

/// Allowed size per kind: (min_w, max_w, min_h, max_h). The list widgets may fill the whole grid.
fn size_limits(kind: &str, cols: i32, rows: i32) -> (i32, i32, i32, i32) {
    match kind {
        "button" | "status" => (1, 1, 1, 1),
        "clock" => (1, 2, 1, 1),
        "sys" => (2, 2, 1, 2),
        "media" => (2, cols, 1, 2),
        _ => (2, cols, 1, rows), // statuses, notifications
    }
}

fn check_look(path: &str, look_label: &Option<String>, icon: &Option<String>, color: &Option<String>, out: &mut Vec<ValidationIssue>) {
    if look_label.as_deref().map(chars).unwrap_or(0) > 24 {
        out.push(issue(format!("{path}/label"), "label longer than 24 characters"));
    }
    if let Some(i) = icon {
        if !is_valid_icon(i) {
            out.push(issue(format!("{path}/icon"), "icon must be lucide:<name>, emoji:<symbol> or img:<16 hex characters>"));
        }
    }
    if let Some(c) = color {
        if !is_color(c) {
            out.push(issue(format!("{path}/color"), "color must be #rrggbb"));
        }
    }
}

pub fn validate_layout(layout: &LayoutFile, actions: &ActionsFile) -> Vec<ValidationIssue> {
    let mut out = Vec::new();
    if layout.version != 1 {
        out.push(issue("/version", "version must be 1"));
    }
    if !(3..=8).contains(&layout.grid.cols) {
        out.push(issue("/grid/cols", "grid columns must be an integer from 3 to 8"));
    }
    if !(2..=5).contains(&layout.grid.rows) {
        out.push(issue("/grid/rows", "grid rows must be an integer from 2 to 5"));
    }
    let (cols, rows) = (i32::from(layout.grid.cols), i32::from(layout.grid.rows));
    if !(1..=10).contains(&layout.pages.len()) {
        out.push(issue("/pages", "pages must contain 1..10 entries"));
    }

    let mut page_ids = HashSet::new();
    for (pi, page) in layout.pages.iter().enumerate() {
        let path = format!("/pages/{pi}");
        if !is_id(&page.id) {
            out.push(issue(format!("{path}/id"), "invalid page id"));
        }
        if !page_ids.insert(page.id.as_str()) {
            out.push(issue(path.clone(), format!("duplicate page id '{}'", page.id)));
        }
        if !(1..=30).contains(&chars(&page.name)) {
            out.push(issue(format!("{path}/name"), "page name must be 1..30 characters"));
        }
        if page.items.len() > 40 {
            out.push(issue(format!("{path}/items"), "at most 40 items per page"));
        }
    }

    // Context pages (DATA_MODEL.md 3.2 item 9): the first page is home; an app belongs to one page only.
    let mut app_owner: std::collections::HashMap<&str, &str> = std::collections::HashMap::new();
    for (pi, page) in layout.pages.iter().enumerate() {
        let Some(apps) = &page.apps else { continue };
        if pi == 0 && !apps.is_empty() {
            out.push(issue("/pages/0/apps", "the first page is the home page and cannot have apps"));
        }
        if apps.len() > 10 {
            out.push(issue(format!("/pages/{pi}/apps"), "at most 10 apps per page"));
        }
        for (ai, app) in apps.iter().enumerate() {
            let path = format!("/pages/{pi}/apps/{ai}");
            if !is_app_name(app) {
                out.push(issue(path.clone(), "app must be a lower-case exe name such as code.exe"));
            }
            match app_owner.get(app.as_str()) {
                Some(owner) if *owner == page.id => out.push(issue(path, format!("duplicate app '{app}'"))),
                Some(_) => out.push(issue(path, format!("{app} is already used by another page"))),
                None => {
                    app_owner.insert(app.as_str(), page.id.as_str());
                }
            }
        }
    }

    let mut item_ids = HashSet::new();
    for (pi, page) in layout.pages.iter().enumerate() {
        for (ii, item) in page.items.iter().enumerate() {
            let path = format!("/pages/{pi}/items/{ii}");
            let (id, x, y, w, h) = item.rect();

            if !is_id(id) {
                out.push(issue(format!("{path}/id"), "invalid item id"));
            }
            if !item_ids.insert(id) {
                out.push(issue(path.clone(), format!("duplicate item id '{id}'")));
            }

            if x < 0 || y < 0 || w < 1 || h < 1 || x + w > cols || y + h > rows {
                out.push(issue(path.clone(), format!("item is outside the {cols}x{rows} grid")));
            }
            for other in &page.items[..ii] {
                let (oid, ox, oy, ow, oh) = other.rect();
                if x < ox + ow && ox < x + w && y < oy + oh && oy < y + h {
                    out.push(issue(path.clone(), format!("item overlaps '{oid}'")));
                    break;
                }
            }
            let (min_w, max_w, min_h, max_h) = size_limits(item.kind_name(), cols, rows);
            if w < min_w || w > max_w || h < min_h || h > max_h {
                out.push(issue(path.clone(), format!("size {w}x{h} is not allowed for kind '{}'", item.kind_name())));
            }

            match item {
                LayoutItem::Button(b) => {
                    check_look(&path, &b.label, &b.icon, &b.color, &mut out);
                    if b.action_id.is_some() == b.navigate.is_some() {
                        out.push(issue(path.clone(), "button needs exactly one of actionId / navigate"));
                    }
                    if let Some(a) = &b.action_id {
                        if !actions.actions.contains_key(a) {
                            out.push(issue(path.clone(), format!("unknown actionId '{a}'")));
                        }
                    }
                    if let Some(n) = &b.navigate {
                        if !page_ids.contains(n.as_str()) {
                            out.push(issue(path.clone(), format!("unknown page '{n}'")));
                        }
                    }
                    if b.confirm.is_some() && b.action_id.is_none() {
                        out.push(issue(path.clone(), "confirm is only allowed with actionId"));
                    }
                    if let Some(bind) = &b.bind {
                        if !is_var_name(&bind.var) {
                            out.push(issue(format!("{path}/bind/var"), "invalid variable name"));
                        }
                        if !(1..=8).contains(&bind.states.len()) {
                            out.push(issue(format!("{path}/bind/states"), "states must contain 1..8 entries"));
                        }
                        for (key, look) in &bind.states {
                            let sp = format!("{path}/bind/states/{}", esc(key));
                            if chars(key) > 40 {
                                out.push(issue(sp.clone(), "state key longer than 40 characters"));
                            }
                            check_look(&sp, &look.label, &look.icon, &look.color, &mut out);
                        }
                    }
                }
                LayoutItem::Statuses(s) => {
                    if s.group.as_deref().map(chars).unwrap_or(0) > 40 {
                        out.push(issue(format!("{path}/group"), "group longer than 40 characters"));
                    }
                    if s.title.as_deref().map(chars).unwrap_or(0) > 30 {
                        out.push(issue(format!("{path}/title"), "title longer than 30 characters"));
                    }
                }
                LayoutItem::Status(s) => {
                    if !is_id(&s.status_id) {
                        out.push(issue(format!("{path}/statusId"), "invalid status id"));
                    }
                }
                LayoutItem::Media(m) => {
                    for (field, id) in [("prevAction", &m.prev_action), ("playAction", &m.play_action), ("nextAction", &m.next_action)] {
                        if let Some(id) = id {
                            if !actions.actions.contains_key(id) {
                                out.push(issue(path.clone(), format!("unknown {field} '{id}'")));
                            }
                        }
                    }
                }
                LayoutItem::Clock(_) | LayoutItem::Sys(_) | LayoutItem::Notifications(_) => {}
            }
        }
    }

    let d = &layout.display;
    if d.dim_after_sec != 0 && !(10..=3600).contains(&d.dim_after_sec) {
        out.push(issue("/display/dimAfterSec", "must be 0 or 10..3600"));
    }
    if !(0.05..=1.0).contains(&d.dim_level) {
        out.push(issue("/display/dimLevel", "must be 0.05..1"));
    }
    if d.sleep_after_sec != 0 && !(30..=86400).contains(&d.sleep_after_sec) {
        out.push(issue("/display/sleepAfterSec", "must be 0 or 30..86400"));
    } else if d.sleep_after_sec != 0 && d.dim_after_sec != 0 && d.sleep_after_sec <= d.dim_after_sec {
        out.push(issue("/display/sleepAfterSec", "must be greater than dimAfterSec"));
    }
    if !is_hhmm(&d.night.from) {
        out.push(issue("/display/night/from", "must be HH:MM"));
    }
    if !is_hhmm(&d.night.to) {
        out.push(issue("/display/night/to", "must be HH:MM"));
    }
    out
}

// ===== config =====

pub fn is_api_token(s: &str) -> bool {
    s.len() == 46 && s.starts_with("dk_") && s[3..].bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
}

pub fn validate_config(c: &ConfigFile) -> Vec<ValidationIssue> {
    let mut out = Vec::new();
    if c.version != 1 {
        out.push(issue("/version", "version must be 1"));
    }
    if !(1..=40).contains(&chars(&c.agent_name)) {
        out.push(issue("/agentName", "agentName must be 1..40 characters"));
    }
    if !(1024..=65535).contains(&c.port) {
        out.push(issue("/port", "port must be 1024..65535"));
    }
    if c.bind != "0.0.0.0" && c.bind != "127.0.0.1" {
        out.push(issue("/bind", "bind must be 0.0.0.0 or 127.0.0.1"));
    }
    if !is_api_token(&c.api_token) {
        out.push(issue("/apiToken", "apiToken must be dk_ followed by 43 base64url characters"));
    }
    if !(1..=60).contains(&c.collectors.sys.interval_sec) {
        out.push(issue("/collectors/sys/intervalSec", "must be 1..60"));
    }
    if c.collectors.checks.len() > 20 {
        out.push(issue("/collectors/checks", "at most 20 checks"));
    }
    let mut ids = HashSet::new();
    for (i, ch) in c.collectors.checks.iter().enumerate() {
        let p = format!("/collectors/checks/{i}");
        if !is_id(&ch.id) {
            out.push(issue(format!("{p}/id"), "invalid check id"));
        }
        if !ids.insert(ch.id.as_str()) {
            out.push(issue(format!("{p}/id"), format!("duplicate check id '{}'", ch.id)));
        }
        if !(1..=60).contains(&chars(&ch.label)) {
            out.push(issue(format!("{p}/label"), "label must be 1..60 characters"));
        }
        if !is_http_url(&ch.url) {
            out.push(issue(format!("{p}/url"), "url must be http or https, up to 2000 characters"));
        }
        if !(10..=3600).contains(&ch.interval_sec) {
            out.push(issue(format!("{p}/intervalSec"), "must be 10..3600"));
        }
        if !(1..=30).contains(&ch.timeout_sec) {
            out.push(issue(format!("{p}/timeoutSec"), "must be 1..30"));
        }
        if let Some(s) = ch.expect_status {
            if !(100..=599).contains(&s) {
                out.push(issue(format!("{p}/expectStatus"), "must be 100..599"));
            }
        }
        if ch.group.as_deref().map(chars).unwrap_or(0) > 40 {
            out.push(issue(format!("{p}/group"), "group longer than 40 characters"));
        }
    }
    out
}
