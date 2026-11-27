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

/// A DNS name for a mail server: letters, digits, dots and hyphens, 1..253 characters, no empty label (never an address with
/// a path, a port or a scheme).
pub fn is_mail_host(s: &str) -> bool {
    (1..=253).contains(&s.len())
        && s.split('.').all(|label| (1..=63).contains(&label.len()) && !label.starts_with('-') && !label.ends_with('-') && label.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-'))
}

/// M15: variables set by the collectors; nobody else writes them (PROTOCOL.md 4, HTTP rules).
pub fn is_reserved_var(name: &str) -> bool {
    ["audio.", "mic.", "media.", "obs.", "ai.", "twitch.", "youtube.", "battery.", "mail.", "notif.", "voice.", "notes.", "ha."].iter().any(|p| name.starts_with(p))
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
        // M15
        || matches!(k, "printscreen" | "insert" | "period" | "comma" | "minus" | "equal" | "slash" | "semicolon")
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
            } else if is_reserved_var(v) {
                out.push(issue(format!("{base}/toggleVar"), format!("toggleVar: \"{v}\" is set by Decka itself — remove it")));
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
            Action::Media { .. } | Action::System { .. } | Action::Audio { .. } | Action::Screenshot { .. } | Action::NotesRecord { .. } => {}
            Action::Ha { entity_id, service, .. } => {
                if !crate::homeassistant::is_entity_id(entity_id) {
                    out.push(issue(format!("{base}/entityId"), "entityId looks like light.bedroom, at most 61 characters"));
                } else if *service == crate::model::HaService::Scene && !entity_id.starts_with("scene.") {
                    out.push(issue(format!("{base}/entityId"), "the service scene needs a scene entity"));
                }
            }
            Action::Http { method, url, body, headers, .. } => {
                if let Err(why) = crate::webhook::check_url(url) {
                    out.push(issue(format!("{base}/url"), why));
                }
                match body {
                    Some(_) if *method == HttpMethod::Get => out.push(issue(format!("{base}/body"), "body is only allowed with POST")),
                    Some(b) if chars(b) > crate::webhook::MAX_BODY => out.push(issue(format!("{base}/body"), "body must be at most 4096 characters")),
                    _ => {}
                }
                if let Some(headers) = headers {
                    if headers.len() > crate::webhook::MAX_HEADERS {
                        out.push(issue(format!("{base}/headers"), "at most 10 headers"));
                    }
                    for (name, value) in headers {
                        if let Err(why) = crate::webhook::check_header(name, value) {
                            out.push(issue(format!("{base}/headers/{}", esc(name)), why));
                        }
                    }
                }
            }
            Action::Obs { op, scene, input, .. } => {
                // `scene` goes with op `scene` and `input` with `input_mute_toggle`, and with nothing else (DATA_MODEL.md 2.3).
                for (field, value, needed) in [("scene", scene, *op == ObsOp::Scene), ("input", input, *op == ObsOp::InputMuteToggle)] {
                    match (value, needed) {
                        (Some(v), true) if !(1..=100).contains(&chars(v)) => {
                            out.push(issue(format!("{base}/{field}"), format!("{field} must be 1..100 characters")))
                        }
                        (None, true) => out.push(issue(format!("{base}/{field}"), format!("{field} is required for this operation"))),
                        (Some(_), false) => out.push(issue(format!("{base}/{field}"), format!("{field} is not allowed for this operation"))),
                        _ => {}
                    }
                }
            }
            Action::Twitch { op, message, .. } => match (message, *op == TwitchOp::Announce) {
                (Some(m), true) if !(1..=500).contains(&chars(m)) => out.push(issue(format!("{base}/message"), "message must be 1..500 characters")),
                (None, true) => out.push(issue(format!("{base}/message"), "message is required for an announcement")),
                (Some(_), false) => out.push(issue(format!("{base}/message"), "message is only allowed for an announcement")),
                _ => {}
            },
            Action::Youtube { op, message, .. } => match (message, *op == YoutubeOp::Chat) {
                (Some(m), true) if !(1..=200).contains(&chars(m)) => out.push(issue(format!("{base}/message"), "message must be 1..200 characters")),
                (None, true) => out.push(issue(format!("{base}/message"), "message is required for a chat message")),
                (Some(_), false) => out.push(issue(format!("{base}/message"), "message is only allowed for a chat message")),
                _ => {}
            },
            Action::Sequence { steps, .. } => {
                if !(1..=10).contains(&steps.len()) {
                    out.push(issue(format!("{base}/steps"), "a sequence has 1..10 steps"));
                }
                let mut pauses = 0;
                for (si, step) in steps.iter().enumerate() {
                    let at = format!("{base}/steps/{si}");
                    match step {
                        SequenceStep::Wait { delay_ms } if !(50..=10000).contains(delay_ms) => out.push(issue(at, "a pause is 50..10000 ms")),
                        SequenceStep::Wait { delay_ms } => pauses += delay_ms,
                        SequenceStep::Run { action_id } => match file.actions.get(action_id) {
                            None => out.push(issue(at, format!("action \"{action_id}\" does not exist"))),
                            Some(Action::Sequence { .. }) => out.push(issue(at, "a sequence cannot contain another sequence")),
                            Some(_) => {}
                        },
                    }
                }
                if pauses > 15000 {
                    out.push(issue(format!("{base}/steps"), "the pauses add up to more than 15000 ms"));
                }
            }
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

/// Allowed size per kind as a box: (min_w, max_w, min_h, max_h). The list widgets may fill the whole grid.
fn size_limits(kind: &str, cols: i32, rows: i32, graph: bool) -> (i32, i32, i32, i32) {
    match kind {
        "button" | "status" | "voice" => (1, 1, 1, 1),
        "clock" => (1, 2, 1, 1),
        "sys" if graph => (2, 4, 1, 2),
        "sys" => (2, 2, 1, 2),
        "media" => (2, cols, 1, 2),
        "volume" => (1, 4, 1, 4),
        "timer" => (1, 2, 1, 2),
        "weather" => (1, 4, 1, 2),
        "obs" => (2, 4, 1, 2),
        "ai" | "twitch" | "youtube" => (2, 4, 1, 2),
        "battery" => (1, 3, 1, 2),
        "mail" | "calendar" => (1, 2, 1, 2),
        _ => (2, cols, 1, rows), // statuses, notifications
    }
}

/// The exact size rule (DATA_MODEL.md 3.2 rule 13): the box of `size_limits` plus the kinds whose sizes are a list.
fn size_allowed(kind: &str, w: i32, h: i32, cols: i32, rows: i32, graph: bool) -> bool {
    let (min_w, max_w, min_h, max_h) = size_limits(kind, cols, rows, graph);
    if w < min_w || w > max_w || h < min_h || h > max_h {
        return false;
    }
    match kind {
        "volume" => (w == 1 && h >= 2) || (h == 1 && w >= 2),
        "timer" => !(w == 1 && h == 2),
        "weather" => if h == 1 { w <= 2 } else { w >= 3 },
        // 2x1, 2x2, 3x2, 4x2 (DATA_MODEL.md 3.2 rule 13).
        "ai" => h == 2 || w == 2,
        // 1x1, 2x1, 2x2, 3x1 (DATA_MODEL.md 10.6) and 1x1, 2x1, 2x2 (10.7).
        "battery" => (w, h) != (1, 2) && (w, h) != (3, 2),
        "mail" | "calendar" => (w, h) != (1, 2),
        _ => true,
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
    // The limits depend on the orientation (DATA_MODEL.md 3.2 rule 1): landscape 3..8 x 2..5, portrait 2..5 x 3..8.
    let portrait = layout.display.orientation.as_deref() == Some("portrait");
    let ((c_min, c_max), (r_min, r_max), name) = if portrait { ((2, 5), (3, 8), "portrait") } else { ((3, 8), (2, 5), "landscape") };
    if !(c_min..=c_max).contains(&layout.grid.cols) || !(r_min..=r_max).contains(&layout.grid.rows) {
        out.push(issue(
            "/grid",
            format!("Grid {}×{} does not fit {name} ({c_min}–{c_max} columns, {r_min}–{r_max} rows)", layout.grid.cols, layout.grid.rows),
        ));
    }
    if layout.display.orientation.as_deref().is_some_and(|o| o != "landscape" && o != "portrait") {
        out.push(issue("/display/orientation", "orientation must be landscape or portrait"));
    }
    if layout.display.window_theme.as_deref().is_some_and(|o| !matches!(o, "system" | "light" | "dark")) {
        out.push(issue("/display/windowTheme", "windowTheme must be system, light or dark"));
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
        // Popup groups (DATA_MODEL.md 3.2 rule 12)
        let is_popup = page.popup == Some(true);
        if is_popup && pi == 0 {
            out.push(issue("/pages/0/popup", "the first page cannot be a popup group"));
        }
        if is_popup && page.apps.as_ref().is_some_and(|a| !a.is_empty()) {
            out.push(issue(format!("{path}/apps"), "a popup group cannot show itself for apps: remove them"));
        }
        if page.stay_open.is_some() && !is_popup {
            out.push(issue(format!("{path}/stayOpen"), "stayOpen only makes sense for a popup group"));
        }
        if let Some(time) = &page.time {
            if !is_hhmm(&time.from) || !is_hhmm(&time.to) || time.from == time.to {
                out.push(issue(format!("{path}/time"), "time needs two different times of day, HH:MM"));
            }
            if is_popup || pi == 0 {
                out.push(issue(format!("{path}/time"), "the home page and popup groups cannot be shown by time"));
            }
        }
    }
    let popup_ids: HashSet<&str> = layout.pages.iter().filter(|p| p.popup == Some(true)).map(|p| p.id.as_str()).collect();

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
            let graph = matches!(item, LayoutItem::Sys(s) if s.graph == Some(true));
            if !size_allowed(item.kind_name(), w, h, cols, rows, graph) {
                out.push(issue(path.clone(), format!("size {w}x{h} is not allowed for kind '{}'", item.kind_name())));
            }

            match item {
                LayoutItem::Button(b) => {
                    check_look(&path, &b.label, &b.icon, &b.color, &mut out);
                    if b.face.as_deref().is_some_and(|f| !["standard", "full"].contains(&f)) {
                        out.push(issue(format!("{path}/face"), "face must be standard or full"));
                    }
                    // Allowed: actionId | navigate | popup | actionId + popup (same texts as shared/src/validate.ts).
                    let (has_action, has_nav, has_popup) = (b.action_id.is_some(), b.navigate.is_some(), b.popup.is_some());
                    if !has_action && !has_nav && !has_popup {
                        out.push(issue(path.clone(), "choose an action or a page for this button"));
                    } else if has_nav && has_action {
                        out.push(issue(path.clone(), "a button can either run an action or open a page, not both"));
                    } else if has_nav && has_popup {
                        out.push(issue(path.clone(), "a button can open a page or a popup group, not both"));
                    }
                    if let Some(a) = &b.action_id {
                        if !actions.actions.contains_key(a) {
                            out.push(issue(path.clone(), format!("action \"{a}\" no longer exists — choose another")));
                        }
                    }
                    // "$back" (DATA_MODEL.md 3.8) names no page; inside a popup group it is refused below like any `navigate`.
                    if let Some(n) = b.navigate.as_ref().filter(|n| n.as_str() != "$back") {
                        if !page_ids.contains(n.as_str()) {
                            out.push(issue(path.clone(), format!("page \"{n}\" no longer exists — choose another")));
                        } else if popup_ids.contains(n.as_str()) {
                            out.push(issue(path.clone(), format!("page \"{n}\" is a popup group — choose a normal page")));
                        }
                    }
                    if let Some(p) = &b.popup {
                        if !page_ids.contains(p.as_str()) {
                            out.push(issue(path.clone(), format!("popup group \"{p}\" no longer exists — choose another")));
                        } else if !popup_ids.contains(p.as_str()) {
                            out.push(issue(path.clone(), format!("page \"{p}\" is not a popup group")));
                        }
                    }
                    if page.popup == Some(true) && (has_nav || has_popup) {
                        out.push(issue(path.clone(), "a key inside a popup group cannot open a page or another group"));
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
                LayoutItem::Volume(v) => {
                    for (field, id) in [("upAction", &v.up_action), ("downAction", &v.down_action), ("muteAction", &v.mute_action)] {
                        if let Some(id) = id {
                            if !actions.actions.contains_key(id) {
                                out.push(issue(path.clone(), format!("unknown {field} '{id}'")));
                            }
                        }
                    }
                }
                LayoutItem::Timer(t) => {
                    if let Some(p) = &t.presets {
                        if !(1..=4).contains(&p.len()) || p.iter().any(|m| !(1..=180).contains(m)) {
                            out.push(issue(format!("{path}/presets"), "presets must be 1..4 whole numbers of minutes, 1..180"));
                        }
                    }
                }
                LayoutItem::Ai(a) => {
                    if a.show.as_deref().is_some_and(|s| !["both", "claude", "codex"].contains(&s)) {
                        out.push(issue(format!("{path}/show"), "show must be both, claude or codex"));
                    }
                }
                LayoutItem::List(l) => {
                    if l.list_id.as_deref().is_some_and(|id| id.len() != 16 || !id.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))) {
                        out.push(issue(format!("{path}/listId"), "listId must be 16 hex characters"));
                    }
                }
                LayoutItem::Clock(_) | LayoutItem::Sys(_) | LayoutItem::Notifications(_) | LayoutItem::Weather(_) | LayoutItem::Obs(_) | LayoutItem::Twitch(_) | LayoutItem::Youtube(_) | LayoutItem::Context(_) | LayoutItem::Voice(_) | LayoutItem::Battery(_) | LayoutItem::Mail(_) | LayoutItem::Calendar(_) | LayoutItem::Reminders(_) => {}
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
    if d.theme.as_deref().is_some_and(|t| !["amoled", "studio", "material", "glass"].contains(&t)) {
        out.push(issue("/display/theme", "theme must be amoled, studio, material or glass"));
    }
    if d.accent.as_deref().is_some_and(|c| !is_color(c)) {
        out.push(issue("/display/accent", "accent must be #rrggbb"));
    }
    if d.key_face.as_deref().is_some_and(|f| !["standard", "full"].contains(&f)) {
        out.push(issue("/display/keyFace", "keyFace must be standard or full"));
    }
    if d.language.as_deref().is_some_and(|l| l != "auto" && !crate::tray_text::LANGUAGES.contains(&l)) {
        out.push(issue("/display/language", format!("language must be auto or one of {}", crate::tray_text::LANGUAGES.join(", "))));
    }
    if d.wallpaper.as_deref().is_some_and(|w| !["aurora", "sunset", "ocean", "graphite", "album"].contains(&w)) {
        out.push(issue("/display/wallpaper", "wallpaper must be aurora, sunset, ocean, graphite or album"));
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
    if let Some(server) = &c.server {
        let name = |h: &String| (1..=253).contains(&h.len()) && h.bytes().all(|b| b.is_ascii_alphanumeric() || b".-_".contains(&b));
        if server.allowed_hosts.len() > 20 || !server.allowed_hosts.iter().all(name) {
            out.push(issue("/server/allowedHosts", "at most 20 host names, without a port or a scheme"));
        }
    }
    if let Some(ha) = &c.ha {
        if let Err(why) = crate::homeassistant::base(&ha.url) {
            out.push(issue("/ha/url", why));
        }
        if !ha.token.starts_with("dpapi:") || chars(&ha.token) > 4000 {
            out.push(issue("/ha/token", "token must be written by the settings window"));
        }
        if ha.entities.len() > crate::homeassistant::MAX_ENTITIES || !ha.entities.iter().all(|e| crate::homeassistant::is_entity_id(e)) {
            out.push(issue("/ha/entities", "at most 100 entity ids"));
        }
    }
    if let Some(w) = &c.weather {
        if !(1..=80).contains(&chars(&w.place)) {
            out.push(issue("/weather/place", "place must be 1..80 characters"));
        }
        if !(-90.0..=90.0).contains(&w.lat) {
            out.push(issue("/weather/lat", "lat must be -90..90"));
        }
        if !(-180.0..=180.0).contains(&w.lon) {
            out.push(issue("/weather/lon", "lon must be -180..180"));
        }
        if w.units != "metric" && w.units != "imperial" {
            out.push(issue("/weather/units", "units must be metric or imperial"));
        }
    }
    if let Some(o) = &c.obs {
        if !(1024..=65535).contains(&o.port) {
            out.push(issue("/obs/port", "port must be 1024..65535"));
        }
        // The password is only ever the DPAPI blob written by `set_obs_password` (DATA_MODEL.md 6.4).
        if o.password.as_deref().is_some_and(|p| !p.starts_with("dpapi:") || chars(p) > 4000) {
            out.push(issue("/obs/password", "password must be written by set_obs_password"));
        }
    }
    if let Some(tw) = &c.twitch {
        // A Twitch client id is 30 letters and digits; a loose limit keeps the field free of anything else.
        if tw.client_id.len() > 64 || !tw.client_id.chars().all(|ch| ch.is_ascii_alphanumeric()) {
            out.push(issue("/twitch/clientId", "clientId must be letters and digits, at most 64"));
        }
        if tw.tokens.as_deref().is_some_and(|t| !t.starts_with("dpapi:") || chars(t) > 8000) {
            out.push(issue("/twitch/tokens", "tokens are written by the sign-in only"));
        }
    }
    if let Some(yt) = &c.youtube {
        // A Google client id looks like `123-abc.apps.googleusercontent.com`.
        if yt.client_id.len() > 200 || !yt.client_id.chars().all(|ch| ch.is_ascii_alphanumeric() || ch == '-' || ch == '.' || ch == '_') {
            out.push(issue("/youtube/clientId", "clientId has characters that a client id does not have"));
        }
        if [&yt.secret, &yt.tokens].iter().any(|s| s.as_deref().is_some_and(|v| !v.starts_with("dpapi:") || chars(v) > 8000)) {
            out.push(issue("/youtube/tokens", "the secret and the tokens are written by Decka only"));
        }
    }
    if let Some(w) = &c.windows_notifications {
        if w.exclude.len() > 50 || w.exclude.iter().any(|a| !(1..=80).contains(&chars(a))) {
            out.push(issue("/windowsNotifications/exclude", "at most 50 apps of 1..80 characters"));
        }
    }
    if let Some(m) = &c.mail {
        if m.accounts.len() > 3 {
            out.push(issue("/mail/accounts", "at most 3 mailboxes"));
        }
        for (i, a) in m.accounts.iter().enumerate() {
            let at = format!("/mail/accounts/{i}");
            if !is_id(&a.id) || m.accounts.iter().take(i).any(|o| o.id == a.id) {
                out.push(issue(format!("{at}/id"), "id must be a unique Id"));
            }
            if !(1..=40).contains(&chars(&a.name)) {
                out.push(issue(format!("{at}/name"), "name must be 1..40 characters"));
            }
            if !is_mail_host(&a.host) {
                out.push(issue(format!("{at}/host"), "host must be a host name"));
            }
            if !(1..=65535).contains(&a.port) {
                out.push(issue(format!("{at}/port"), "port must be 1..65535"));
            }
            if !(1..=254).contains(&chars(&a.user)) || a.user.chars().any(|ch| ch.is_control()) {
                out.push(issue(format!("{at}/user"), "user must be 1..254 characters"));
            }
            // The password is only ever the DPAPI blob written by `set_mail_password`.
            if a.password.as_deref().is_some_and(|p| !p.starts_with("dpapi:") || chars(p) > 4000) {
                out.push(issue(format!("{at}/password"), "password must be written by set_mail_password"));
            }
            if a.open_action_id.as_deref().is_some_and(|id| !is_id(id)) {
                out.push(issue(format!("{at}/openActionId"), "openActionId must be an Id"));
            }
        }
    }
    if let Some(a) = &c.ai {
        if ![60, 120, 300].contains(&a.claude.timeout_sec) {
            out.push(issue("/ai/claude/timeoutSec", "timeoutSec must be 60, 120 or 300"));
        }
        if a.hook_token.as_deref().is_some_and(|t| chars(t) > 100) {
            out.push(issue("/ai/hookToken", "hookToken is too long"));
        }
        if let Some(r) = &a.replies {
            if r.len() > 4 || r.iter().any(|t| !(1..=120).contains(&chars(t))) {
                out.push(issue("/ai/replies", "replies are at most 4 texts of 1..120 characters"));
            }
        }
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
