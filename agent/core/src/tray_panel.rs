//! T32.3: the mini panel of the tray - where its window stands, when it goes away, and what it shows. The window itself is
//! made by the application: it never becomes active (like the island), so the click outside and Esc are noticed here.

use serde::Serialize;
use serde_json::{json, Value};

use crate::model::{Action, LayoutItem, VarValue};
use crate::Agent;

/// Logical pixels.
pub const WIDTH: f64 = 320.0;
pub const HEIGHT: f64 = 420.0;
/// The free space to the edges of the work area.
pub const MARGIN: f64 = 8.0;
/// A hidden panel is destroyed after this long.
pub const KEEP: std::time::Duration = std::time::Duration::from_secs(30);
/// A click on the tray icon right after the panel went away is the click that closed it.
pub const REOPEN_GUARD: std::time::Duration = std::time::Duration::from_millis(400);
pub const MAX_FAVORITES: usize = 6;

/// A rectangle of the screen in physical pixels.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Area {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

impl Area {
    pub fn contains(&self, x: i32, y: i32) -> bool {
        x >= self.x && x < self.x + self.width && y >= self.y && y < self.y + self.height
    }
}

/// The top-left corner of a panel of `size` opened by a click at `click`: centred on the click, inside the work area, at the
/// edge (top or bottom) the click is nearer to - where the taskbar is.
pub fn place(click: (i32, i32), work: Area, size: (i32, i32), margin: i32) -> (i32, i32) {
    let clamp = |v: i32, low: i32, high: i32| v.min(high).max(low);
    let x = clamp(click.0 - size.0 / 2, work.x + margin, work.x + work.width - size.0 - margin);
    let y = if click.1 >= work.y + work.height / 2 { work.y + work.height - size.1 - margin } else { work.y + margin };
    (x, clamp(y, work.y, work.y + (work.height - size.1).max(0)))
}

/// What the mouse and the keyboard do now.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Input {
    pub button_down: bool,
    pub escape: bool,
    pub cursor: (i32, i32),
}

/// Whether the panel goes away: Esc, or a button that was just pressed outside it.
pub fn dismisses(before: Input, now: Input, panel: Area) -> bool {
    (now.escape && !before.escape) || (now.button_down && !before.button_down && !panel.contains(now.cursor.0, now.cursor.1))
}

/// The state of the buttons and the cursor (left or right button; Esc).
pub fn input() -> Input {
    #[cfg(windows)]
    {
        use windows::Win32::Foundation::POINT;
        use windows::Win32::UI::Input::KeyboardAndMouse::{GetAsyncKeyState, VK_ESCAPE, VK_LBUTTON, VK_RBUTTON};
        use windows::Win32::UI::WindowsAndMessaging::GetCursorPos;
        // SAFETY: plain queries; the point is a valid out parameter.
        unsafe {
            let down = |key: u16| GetAsyncKeyState(i32::from(key)) as u16 & 0x8000 != 0;
            let mut point = POINT::default();
            let cursor = if GetCursorPos(&mut point).is_ok() { (point.x, point.y) } else { (0, 0) };
            Input { button_down: down(VK_LBUTTON.0) || down(VK_RBUTTON.0), escape: down(VK_ESCAPE.0), cursor }
        }
    }
    #[cfg(not(windows))]
    {
        Input::default()
    }
}

/// The rectangle of a window of this process.
pub fn window_area(hwnd: isize) -> Option<Area> {
    #[cfg(windows)]
    {
        use windows::Win32::Foundation::{HWND, RECT};
        use windows::Win32::UI::WindowsAndMessaging::GetWindowRect;
        let mut r = RECT::default();
        // SAFETY: `r` is a valid out parameter; a window that is gone gives an error.
        unsafe { GetWindowRect(HWND(hwnd as *mut _), &mut r) }.ok()?;
        Some(Area { x: r.left, y: r.top, width: r.right - r.left, height: r.bottom - r.top })
    }
    #[cfg(not(windows))]
    {
        let _ = hwnd;
        None
    }
}

/// One key of the panel.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Favorite {
    pub id: String,
    pub label: String,
    pub icon: String,
    pub active: bool,
    /// A second click is needed: the key of the deck asks "sure?", or it turns the PC off.
    pub confirm: bool,
}

/// The ids the panel shows: the chosen ones that still exist; nothing chosen yet = the first keys of the first page.
pub fn favorite_ids(agent: &Agent) -> Vec<String> {
    let actions = agent.actions();
    if let Some(tray) = agent.config().tray {
        return tray.favorites.into_iter().filter(|id| actions.actions.contains_key(id)).take(MAX_FAVORITES).collect();
    }
    let mut out: Vec<String> = Vec::new();
    for item in agent.layout().pages.first().map(|p| p.items.as_slice()).unwrap_or_default() {
        if let LayoutItem::Button(b) = item {
            if let Some(id) = b.action_id.as_ref().filter(|id| actions.actions.contains_key(*id) && !out.contains(id)) {
                out.push(id.clone());
            }
        }
    }
    out.truncate(MAX_FAVORITES);
    out
}

fn is_on(value: Option<&VarValue>) -> bool {
    matches!(value, Some(VarValue::Bool(true)))
}

/// The favourites as the panel draws them: the caption and the picture of the key of the deck that runs the action.
pub fn favorites(agent: &Agent) -> Vec<Favorite> {
    let (actions, layout) = (agent.actions(), agent.layout());
    let vars: std::collections::BTreeMap<String, VarValue> = serde_json::from_value(agent.store().get("vars")).unwrap_or_default();
    favorite_ids(agent)
        .into_iter()
        .filter_map(|id| {
            let action = actions.actions.get(&id)?;
            let button = layout.pages.iter().flat_map(|p| &p.items).find_map(|item| match item {
                LayoutItem::Button(b) if b.action_id.as_deref() == Some(id.as_str()) => Some(b),
                _ => None,
            });
            let dangerous = matches!(action, Action::System { op, .. } if crate::safety::is_dangerous_system(*op));
            Some(Favorite {
                label: button.and_then(|b| b.label.clone()).filter(|l| !l.is_empty()).unwrap_or_else(|| action.name().to_string()),
                icon: button.and_then(|b| b.icon.clone()).unwrap_or_else(|| "lucide:zap".to_string()),
                active: is_on(action.toggle_var().and_then(|v| vars.get(v))),
                confirm: dangerous || button.is_some_and(|b| b.confirm == Some(true)),
                id,
            })
        })
        .collect()
}

/// The next event of the calendar for the panel: the first one with a time that has not ended, else the first of all.
pub fn next_event(calendar: &Value, unix_ms: i64) -> Value {
    let events = calendar.get("events").and_then(Value::as_array).map(Vec::as_slice).unwrap_or_default();
    let ahead = |e: &&Value| e.get("end").and_then(Value::as_i64).is_some_and(|end| end > unix_ms);
    events.iter().filter(ahead).find(|e| e.get("allDay").and_then(Value::as_bool) != Some(true)).or_else(|| events.iter().find(ahead)).cloned().unwrap_or(Value::Null)
}

/// `tray_state`: everything the panel shows. Reads the sound devices, so not for an async thread.
pub fn state(agent: &Agent) -> Value {
    let store = agent.store();
    let audio = crate::audio::read_state();
    json!({
        "media": store.get("media"),
        "volume": audio.volume,
        "muted": audio.out_muted,
        "micMuted": audio.mic_muted,
        "favorites": favorites(agent),
        "nextEvent": next_event(&store.get("calendar"), crate::auth::now_ms() as i64),
        "icons": store.get("icons"),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const WORK: Area = Area { x: 0, y: 0, width: 1920, height: 1032 };

    #[test]
    fn the_panel_stands_above_the_taskbar_and_stays_on_the_screen() {
        assert_eq!(place((1800, 1060), WORK, (320, 420), 8), (1592, 604));
        assert_eq!(place((900, 1060), WORK, (320, 420), 8), (740, 604));
        // A taskbar at the top, and a second monitor to the left.
        assert_eq!(place((10, 5), WORK, (320, 420), 8), (8, 8));
        let left = Area { x: -1280, y: 0, width: 1280, height: 984 };
        assert_eq!(place((-20, 1000), left, (320, 420), 8), (-328, 556));
    }

    #[test]
    fn a_new_press_outside_or_escape_closes_it() {
        let panel = Area { x: 100, y: 100, width: 320, height: 420 };
        let idle = Input::default();
        let press = |x, y| Input { button_down: true, cursor: (x, y), ..idle };
        assert!(dismisses(idle, press(50, 50), panel));
        assert!(!dismisses(idle, press(200, 200), panel));
        assert!(!dismisses(press(200, 200), press(50, 50), panel), "a drag that began inside, a volume slider");
        assert!(dismisses(idle, Input { escape: true, ..idle }, panel));
        assert!(!dismisses(Input { escape: true, ..idle }, Input { escape: true, ..idle }, panel));
    }

    #[test]
    fn the_next_event_prefers_one_with_a_time() {
        let calendar = serde_json::json!({ "events": [
            { "id": "a", "allDay": true, "end": 500 },
            { "id": "b", "allDay": false, "end": 50 },
            { "id": "c", "allDay": false, "end": 400 },
        ] });
        assert_eq!(next_event(&calendar, 100)["id"], "c");
        assert_eq!(next_event(&calendar, 450)["id"], "a");
        assert_eq!(next_event(&calendar, 600), Value::Null);
        assert_eq!(next_event(&Value::Null, 0), Value::Null);
    }
}
