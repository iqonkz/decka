//! A snapshot of the interactive elements of the active window (T27.1). It is the only thing the screen guide may point at: the
//! model never names an element, it picks an `id` from this list (the `enum` of its JSON schema). Read through UI Automation on a
//! thread of its own with a short deadline; nothing is written to disk.

use std::time::{Duration, Instant};

use serde::Serialize;

/// Most elements in a snapshot.
pub const MAX_ELEMENTS: usize = 200;
/// Deepest level of the tree that is looked at (the window itself is level 0).
pub const MAX_DEPTH: usize = 12;
/// The whole snapshot is given up after this long.
pub const TIMEOUT: Duration = Duration::from_millis(300);
/// A name longer than this is cut (it only has to be recognised).
const MAX_NAME: usize = 100;

/// A rectangle on the screen in physical pixels.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Default)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

/// One node as the system reports it, before the rules below are applied.
#[derive(Debug, Clone, Default)]
pub struct RawNode {
    pub role: String,
    pub name: String,
    pub automation_id: String,
    pub rect: Rect,
    /// Not hidden and on the screen.
    pub visible: bool,
    pub enabled: bool,
    /// Something a person can press, type into or choose (a button, a field, a tab, a menu item...).
    pub interactive: bool,
    pub children: Vec<RawNode>,
}

/// An element of the snapshot; `id` is its number in this snapshot (from 1) and means nothing in another one.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Element {
    pub id: u32,
    pub role: String,
    pub name: String,
    pub automation_id: String,
    pub rect: Rect,
    pub parent_name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Default)]
pub struct Snapshot {
    pub elements: Vec<Element>,
    /// The list was cut by a limit or by the deadline.
    pub truncated: bool,
    /// Some elements are blocks of text seen in a picture of the window (T30.6), not parts UI Automation knows.
    pub text: bool,
}

impl Snapshot {
    /// The ids, for the `enum` of a schema.
    pub fn ids(&self) -> Vec<u32> {
        self.elements.iter().map(|e| e.id).collect()
    }

    pub fn get(&self, id: u32) -> Option<&Element> {
        self.elements.iter().find(|e| e.id == id)
    }
}

fn keeps(n: &RawNode) -> bool {
    n.interactive && n.visible && n.enabled && n.rect.width > 0 && n.rect.height > 0
}

fn cut(s: &str) -> String {
    s.trim().chars().take(MAX_NAME).collect()
}

/// Walks the tree depth first: hidden, disabled and empty elements are dropped, at most `MAX_ELEMENTS` are kept, nothing deeper than
/// `MAX_DEPTH` is looked at, and `expired` (the deadline) ends the walk. A hidden node hides its whole branch.
pub fn build(root: &RawNode, expired: &dyn Fn() -> bool) -> Snapshot {
    let mut out = Snapshot::default();
    walk(root, 0, "", expired, &mut out);
    out
}

fn walk(n: &RawNode, depth: usize, parent: &str, expired: &dyn Fn() -> bool, out: &mut Snapshot) {
    if out.truncated {
        return;
    }
    if expired() {
        out.truncated = true;
        return;
    }
    if !n.visible {
        return;
    }
    if keeps(n) {
        if out.elements.len() >= MAX_ELEMENTS {
            out.truncated = true;
            return;
        }
        out.elements.push(Element {
            id: out.elements.len() as u32 + 1,
            role: n.role.clone(),
            name: cut(&n.name),
            automation_id: cut(&n.automation_id),
            rect: n.rect,
            parent_name: cut(parent),
        });
    }
    if depth >= MAX_DEPTH {
        if !n.children.is_empty() {
            out.truncated = true;
        }
        return;
    }
    let own = if n.name.trim().is_empty() { parent } else { n.name.as_str() };
    for c in &n.children {
        walk(c, depth + 1, own, expired, out);
    }
}

/// The snapshot of the window that has the focus, or `None` (no window, not Windows, an error or the deadline before anything was read).
pub fn active_window() -> Option<Snapshot> {
    window(None)
}

/// The snapshot of the window with this handle; `None` = the one that has the focus.
pub fn window(handle: Option<isize>) -> Option<Snapshot> {
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let _ = tx.send(read_window(handle));
    });
    rx.recv_timeout(TIMEOUT + Duration::from_millis(200)).ok().flatten()
}

#[cfg(windows)]
fn read_window(handle: Option<isize>) -> Option<Snapshot> {
    use windows::Win32::System::Com::{CoCreateInstance, CoInitializeEx, CoUninitialize, CLSCTX_INPROC_SERVER, COINIT_MULTITHREADED};
    use windows::Win32::UI::Accessibility::{CUIAutomation, IUIAutomation, IUIAutomationElement, IUIAutomationTreeWalker};
    use windows::Win32::Foundation::HWND;
    use windows::Win32::UI::WindowsAndMessaging::GetForegroundWindow;

    fn role(control_type: i32) -> (&'static str, bool) {
        // UIA_*ControlTypeId: Button 50000, CheckBox 50002, ComboBox 50003, Edit 50004, Hyperlink 50005, ListItem 50007, MenuItem 50011,
        // RadioButton 50013, Slider 50015, Spinner 50016, SplitButton 50031, Tab 50018, TabItem 50019, TreeItem 50024, Document 50030.
        match control_type {
            50000 => ("button", true),
            50002 => ("checkbox", true),
            50003 => ("combobox", true),
            50004 => ("edit", true),
            50005 => ("link", true),
            50007 => ("listitem", true),
            50011 => ("menuitem", true),
            50013 => ("radiobutton", true),
            50015 => ("slider", true),
            50016 => ("spinner", true),
            50019 => ("tab", true),
            50024 => ("treeitem", true),
            50031 => ("splitbutton", true),
            50030 => ("document", false),
            _ => ("other", false),
        }
    }

    fn node(walker: &IUIAutomationTreeWalker, el: &IUIAutomationElement, depth: usize, started: Instant, budget: &mut usize) -> RawNode {
        // SAFETY: COM calls on the thread that initialised COM; the objects are local to this walk.
        unsafe {
            let (role, interactive) = role(el.CurrentControlType().map(|t| t.0).unwrap_or(0));
            let r = el.CurrentBoundingRectangle().unwrap_or_default();
            let mut n = RawNode {
                role: role.to_string(),
                name: el.CurrentName().map(|s| s.to_string()).unwrap_or_default(),
                automation_id: el.CurrentAutomationId().map(|s| s.to_string()).unwrap_or_default(),
                rect: Rect { x: r.left, y: r.top, width: r.right - r.left, height: r.bottom - r.top },
                visible: !el.CurrentIsOffscreen().map(|b| b.as_bool()).unwrap_or(false),
                enabled: el.CurrentIsEnabled().map(|b| b.as_bool()).unwrap_or(true),
                interactive,
                children: Vec::new(),
            };
            if depth >= MAX_DEPTH || !n.visible || started.elapsed() > TIMEOUT || *budget == 0 {
                return n;
            }
            let mut child = walker.GetFirstChildElement(el).ok();
            while let Some(c) = child {
                if started.elapsed() > TIMEOUT || *budget == 0 {
                    break;
                }
                *budget -= 1;
                n.children.push(node(walker, &c, depth + 1, started, budget));
                child = walker.GetNextSiblingElement(&c).ok();
            }
            n
        }
    }

    // SAFETY: COM on a thread of its own; every COM object is dropped inside `read` before `CoUninitialize`.
    unsafe {
        let initialised = CoInitializeEx(None, COINIT_MULTITHREADED).is_ok();
        let read = || -> Option<Snapshot> {
            let hwnd = handle.map_or_else(|| GetForegroundWindow(), |h| HWND(h as *mut core::ffi::c_void));
            if hwnd.is_invalid() {
                return None;
            }
            let automation: IUIAutomation = CoCreateInstance(&CUIAutomation, None, CLSCTX_INPROC_SERVER).ok()?;
            let root = automation.ElementFromHandle(hwnd).ok()?;
            let walker = automation.ControlViewWalker().ok()?;
            let started = Instant::now();
            let mut budget = MAX_ELEMENTS * 8;
            let tree = node(&walker, &root, 0, started, &mut budget);
            // The walk stops at the deadline by itself; what it read is kept (it used to be thrown away whole).
            let late = started.elapsed() > TIMEOUT || budget == 0;
            let mut snapshot = build(&tree, &|| false);
            snapshot.truncated |= late;
            Some(snapshot)
        };
        let out = read();
        if initialised {
            CoUninitialize();
        }
        out
    }
}

#[cfg(not(windows))]
fn read_window(_: Option<isize>) -> Option<Snapshot> {
    None
}

/// A mouse click in the middle of `rect`; the cursor goes back to where it was.
pub fn click_centre(rect: Rect) -> Result<String, String> {
    #[cfg(windows)]
    {
        mouse_click((rect.x + rect.width / 2, rect.y + rect.height / 2));
        Ok("Pressed".into())
    }
    #[cfg(not(windows))]
    {
        let _ = rect;
        Err("unsupported platform".into())
    }
}

#[cfg(windows)]
fn mouse_click((x, y): (i32, i32)) {
    use windows::Win32::Foundation::POINT;
    use windows::Win32::UI::Input::KeyboardAndMouse::{mouse_event, MOUSEEVENTF_LEFTDOWN, MOUSEEVENTF_LEFTUP};
    use windows::Win32::UI::WindowsAndMessaging::{GetCursorPos, SetCursorPos};
    // SAFETY: plain input calls of this process.
    unsafe {
        let mut before = POINT::default();
        let _ = GetCursorPos(&mut before);
        let _ = SetCursorPos(x, y);
        mouse_event(MOUSEEVENTF_LEFTDOWN, 0, 0, 0, 0);
        mouse_event(MOUSEEVENTF_LEFTUP, 0, 0, 0, 0);
        let _ = SetCursorPos(before.x, before.y);
    }
}

/// Acts on the element at `rect` (T27.4): a click is the `Invoke` of the element when it has one, else a mouse click in the centre
/// of the rectangle; a scroll is the mouse wheel over it; closing a tab is Ctrl+W of the active program. Blocking, short.
pub fn perform(rect: Rect, action: super::screenhelp::UiAction) -> Result<String, String> {
    #[cfg(windows)]
    {
        win_perform(rect, action)
    }
    #[cfg(not(windows))]
    {
        let _ = (rect, action);
        Err("unsupported platform".into())
    }
}

#[cfg(windows)]
fn win_perform(rect: Rect, action: super::screenhelp::UiAction) -> Result<String, String> {
    use super::screenhelp::UiAction;
    use windows::Win32::Foundation::POINT;
    use windows::Win32::System::Com::{CoCreateInstance, CoInitializeEx, CoUninitialize, CLSCTX_INPROC_SERVER, COINIT_MULTITHREADED};
    use windows::Win32::UI::Accessibility::{CUIAutomation, IUIAutomation, IUIAutomationInvokePattern, UIA_InvokePatternId};
    use windows::Win32::UI::Input::KeyboardAndMouse::{keybd_event, mouse_event, KEYEVENTF_KEYUP, MOUSEEVENTF_WHEEL, VK_CONTROL};
    use windows::Win32::UI::WindowsAndMessaging::{GetCursorPos, SetCursorPos};

    let (cx, cy) = (rect.x + rect.width / 2, rect.y + rect.height / 2);
    // SAFETY: plain input calls of this process; COM is initialised and released on this thread.
    unsafe {
        match action {
            UiAction::CloseTab => {
                keybd_event(VK_CONTROL.0 as u8, 0, Default::default(), 0);
                keybd_event(b'W', 0, Default::default(), 0);
                keybd_event(b'W', 0, KEYEVENTF_KEYUP, 0);
                keybd_event(VK_CONTROL.0 as u8, 0, KEYEVENTF_KEYUP, 0);
                Ok("Tab closed".into())
            }
            UiAction::ScrollDown | UiAction::ScrollUp => {
                let mut before = POINT::default();
                let _ = GetCursorPos(&mut before);
                let _ = SetCursorPos(cx, cy);
                let delta: i32 = if action == UiAction::ScrollDown { -360 } else { 360 };
                mouse_event(MOUSEEVENTF_WHEEL, 0, 0, delta, 0);
                let _ = SetCursorPos(before.x, before.y);
                Ok("Scrolled".into())
            }
            UiAction::Click => {
                let initialised = CoInitializeEx(None, COINIT_MULTITHREADED).is_ok();
                let invoked = (|| -> Option<()> {
                    let automation: IUIAutomation = CoCreateInstance(&CUIAutomation, None, CLSCTX_INPROC_SERVER).ok()?;
                    let element = automation.ElementFromPoint(POINT { x: cx, y: cy }).ok()?;
                    let pattern: IUIAutomationInvokePattern = element.GetCurrentPatternAs(UIA_InvokePatternId).ok()?;
                    pattern.Invoke().ok()
                })();
                if initialised {
                    CoUninitialize();
                }
                if invoked.is_none() {
                    mouse_click((cx, cy));
                }
                Ok("Pressed".into())
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn button(name: &str) -> RawNode {
        RawNode { role: "button".into(), name: name.into(), rect: Rect { x: 10, y: 10, width: 80, height: 24 }, visible: true, enabled: true, interactive: true, ..Default::default() }
    }

    fn group(name: &str, children: Vec<RawNode>) -> RawNode {
        RawNode { role: "other".into(), name: name.into(), rect: Rect { x: 0, y: 0, width: 500, height: 500 }, visible: true, enabled: true, interactive: false, children, ..Default::default() }
    }

    fn never() -> bool {
        false
    }

    #[test]
    fn only_visible_enabled_sized_interactive_elements_are_kept_and_numbered_from_one() {
        let hidden = RawNode { visible: false, ..button("hidden") };
        let off = RawNode { enabled: false, ..button("off") };
        let flat = RawNode { rect: Rect::default(), ..button("flat") };
        let label = RawNode { interactive: false, ..button("label") };
        let tree = group("Window", vec![button("Save"), hidden, off, flat, label, button("Cancel")]);
        let s = build(&tree, &never);
        assert_eq!(s.elements.iter().map(|e| (e.id, e.name.as_str())).collect::<Vec<_>>(), vec![(1, "Save"), (2, "Cancel")]);
        assert_eq!(s.ids(), vec![1, 2]);
        assert!(!s.truncated);
        assert_eq!(s.get(2).unwrap().parent_name, "Window");
        assert!(s.get(3).is_none(), "an id outside the snapshot is nothing");
    }

    #[test]
    fn a_hidden_branch_hides_its_children() {
        let tree = group("W", vec![RawNode { visible: false, children: vec![button("inside")], ..group("Hidden pane", vec![]) }]);
        assert!(build(&tree, &never).elements.is_empty());
    }

    #[test]
    fn at_most_two_hundred_elements_are_kept() {
        let tree = group("W", (0..250).map(|i| button(&format!("b{i}"))).collect());
        let s = build(&tree, &never);
        assert_eq!(s.elements.len(), MAX_ELEMENTS);
        assert!(s.truncated);
        assert_eq!(s.elements.last().unwrap().id, 200);
    }

    #[test]
    fn nothing_deeper_than_twelve_levels_is_looked_at() {
        let mut deep = button("too deep");
        for i in 0..MAX_DEPTH {
            deep = group(&format!("g{i}"), vec![deep]);
        }
        // `deep` now sits at level 12 inside 12 groups: the groups are levels 0..11, the button is level 12 - still read.
        assert_eq!(build(&deep, &never).elements.len(), 1);
        let deeper = group("one more", vec![deep]);
        let s = build(&deeper, &never);
        assert!(s.elements.is_empty(), "level 13 is not read");
        assert!(s.truncated);
    }

    #[test]
    fn the_deadline_ends_the_walk_and_marks_the_list_cut() {
        let tree = group("W", vec![button("a"), button("b"), button("c")]);
        let calls = std::cell::Cell::new(0);
        let s = build(&tree, &|| {
            calls.set(calls.get() + 1);
            calls.get() > 2
        });
        assert_eq!(s.elements.len(), 1, "the root and the first child were read, then time ran out");
        assert!(s.truncated);
    }

    #[test]
    fn names_are_trimmed_and_cut() {
        let long = "x".repeat(300);
        let s = build(&group("W", vec![button(&format!("  {long}  "))]), &never);
        assert_eq!(s.elements[0].name.chars().count(), 100);
    }

    #[test]
    fn reading_the_active_window_never_hangs() {
        let started = Instant::now();
        let _ = active_window();
        assert!(started.elapsed() < TIMEOUT + Duration::from_millis(900));
    }
}
