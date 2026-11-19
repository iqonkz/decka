//! Windows notifications on the phone (DATA_MODEL.md 10.4): `UserNotificationListener` read in a loop, new toasts become
//! `notifications` of the phone. The phone sends nothing but the id of a notification it wants opened (ARCHITECTURE.md S21).

use std::collections::{BTreeSet, HashSet};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::Duration;

use crate::model::Action;

/// How often the list of toasts is read (the event is not available to an unpackaged app).
pub const POLL: Duration = Duration::from_secs(3);
/// Cuts of the texts shown on the phone.
pub const MAX_TITLE: usize = 120;
pub const MAX_BODY: usize = 500;
/// How many different apps are remembered for the list of exceptions.
pub const MAX_APPS: usize = 50;

/// One toast of Windows as read: the id Windows gave it, the app, its AUMID and the texts of the `ToastGeneric` binding.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WinToast {
    pub id: u32,
    pub app: String,
    pub aumid: String,
    pub texts: Vec<String>,
}

fn cut(text: &str, max: usize) -> String {
    text.chars().take(max).collect()
}

/// The first text is the title, the rest (separated by a line break) the body. Empty texts are skipped; nothing left = `None`.
/// Titles are cut to 120 characters, bodies to 500.
pub fn title_and_body(texts: &[String]) -> Option<(String, Option<String>)> {
    let mut parts = texts.iter().map(|t| t.trim()).filter(|t| !t.is_empty());
    let title = cut(parts.next()?, MAX_TITLE);
    let rest: Vec<&str> = parts.collect();
    let body = (!rest.is_empty()).then(|| cut(&rest.join("\n"), MAX_BODY));
    Some((title, body))
}

fn unescape(text: &str) -> String {
    text.replace("&lt;", "<").replace("&gt;", ">").replace("&quot;", "\"").replace("&apos;", "'").replace("&amp;", "&")
}

/// The texts of the `ToastGeneric` binding in the XML of a toast, in order. Texts marked `placement="attribution"` (the
/// "via ..." line) are left out. A small reader, enough for the shape toasts have.
pub fn texts_from_toast_xml(xml: &str) -> Vec<String> {
    let Some(start) = xml.find("template=\"ToastGeneric\"") else { return Vec::new() };
    let rest = &xml[start..];
    let end = rest.find("</binding>").unwrap_or(rest.len());
    let binding = &rest[..end];
    let mut out = Vec::new();
    let mut at = 0;
    while let Some(open) = binding[at..].find("<text") {
        let tag_start = at + open;
        let Some(tag_end) = binding[tag_start..].find('>') else { break };
        let tag = &binding[tag_start..tag_start + tag_end];
        let content_start = tag_start + tag_end + 1;
        if tag.ends_with('/') {
            at = content_start;
            continue;
        }
        let Some(close) = binding[content_start..].find("</text>") else { break };
        if !tag.contains("placement=\"attribution\"") {
            out.push(unescape(&binding[content_start..content_start + close]));
        }
        at = content_start + close + "</text>".len();
    }
    out
}

/// Is `app` (its display name) in the list of exceptions? Names are compared without regard to case.
pub fn is_excluded(app: &str, exclude: &[String]) -> bool {
    exclude.iter().any(|e| e.eq_ignore_ascii_case(app))
}

/// An AUMID is `Publisher.App_hash!AppId` or a path-like id of a classic program: letters, digits and `. _ ! - \ :`, at most
/// 256 characters. Anything else is not launched.
pub fn valid_aumid(aumid: &str) -> bool {
    !aumid.is_empty() && aumid.len() <= 256 && aumid.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '!' | '-' | '\\' | ':' | ' ' | '{' | '}'))
}

/// `explorer.exe shell:AppsFolder\<AUMID>` as an action for the runner (no shell; it opens or brings forward the program).
pub fn open_action(aumid: &str) -> Option<Action> {
    valid_aumid(aumid).then(|| Action::Launch {
        name: "Open notification".to_string(),
        toggle_var: None,
        program: "explorer.exe".to_string(),
        args: Some(vec![format!("shell:AppsFolder\\{aumid}")]),
        cwd: None,
        hidden: None,
    })
}

/// What changed between two reads: the toasts that are new and the ids that are gone.
pub fn diff(seen: &HashSet<u32>, now: &[WinToast]) -> (Vec<WinToast>, Vec<u32>) {
    let current: HashSet<u32> = now.iter().map(|t| t.id).collect();
    let added = now.iter().filter(|t| !seen.contains(&t.id)).cloned().collect();
    let mut removed: Vec<u32> = seen.difference(&current).copied().collect();
    removed.sort_unstable();
    (added, removed)
}

/// The apps seen so far, for the list of exceptions, newest additions kept up to `MAX_APPS`.
pub fn remember_app(apps: &mut BTreeSet<String>, app: &str) {
    if !app.trim().is_empty() && (apps.len() < MAX_APPS || apps.contains(app)) {
        apps.insert(app.to_string());
    }
}

/// What the window shows next to the switch: `allowed`, `denied` (blocked in Windows settings) or `unknown`.
pub fn access_status() -> &'static str {
    #[cfg(windows)]
    {
        use windows::UI::Notifications::Management::{UserNotificationListener, UserNotificationListenerAccessStatus};
        return match UserNotificationListener::Current().and_then(|l| l.GetAccessStatus()) {
            Ok(s) if s == UserNotificationListenerAccessStatus::Allowed => "allowed",
            Ok(s) if s == UserNotificationListenerAccessStatus::Denied => "denied",
            _ => "unknown",
        };
    }
    #[cfg(not(windows))]
    "unknown"
}

/// Handlers of the collector: a toast appeared, a toast went away.
pub struct Handlers {
    pub added: Box<dyn Fn(WinToast) + Send + Sync>,
    pub removed: Box<dyn Fn(u32) + Send + Sync>,
    /// Every app seen, also in the first read (those toasts are not shown, only remembered).
    pub seen_app: Box<dyn Fn(&str) + Send + Sync>,
}

/// A running reader thread; dropped to end it.
pub struct WinNotifCollector {
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl WinNotifCollector {
    pub fn start(handlers: Handlers) -> WinNotifCollector {
        let stop = Arc::new(AtomicBool::new(false));
        let flag = stop.clone();
        let thread = std::thread::Builder::new()
            .name("decka-winnotif".to_string())
            .spawn(move || run(handlers, flag))
            .expect("spawn winnotif thread");
        WinNotifCollector { stop, thread: Some(thread) }
    }
}

impl Drop for WinNotifCollector {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

fn sleep_unless_stopped(total: Duration, stop: &AtomicBool) -> bool {
    let end = std::time::Instant::now() + total;
    while std::time::Instant::now() < end {
        if stop.load(Ordering::Relaxed) {
            return false;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    !stop.load(Ordering::Relaxed)
}

fn run(handlers: Handlers, stop: Arc<AtomicBool>) {
    #[cfg(windows)]
    win::init_and_request_access();
    let mut seen: HashSet<u32> = HashSet::new();
    let mut first = true;
    loop {
        match read_toasts() {
            Ok(now) => {
                let (added, removed) = diff(&seen, &now);
                for toast in &added {
                    (handlers.seen_app)(&toast.app);
                }
                // The toasts that were already there when the switch went on are not sent: they are old news.
                if !first {
                    added.into_iter().for_each(|t| (handlers.added)(t));
                    removed.into_iter().for_each(|id| (handlers.removed)(id));
                }
                seen = now.iter().map(|t| t.id).collect();
                first = false;
            }
            Err(e) => tracing::debug!("windows notifications: {e}"),
        }
        if !sleep_unless_stopped(POLL, &stop) {
            return;
        }
    }
}

#[cfg(windows)]
fn read_toasts() -> Result<Vec<WinToast>, String> {
    win::read_toasts()
}

#[cfg(not(windows))]
fn read_toasts() -> Result<Vec<WinToast>, String> {
    Err("not Windows".to_string())
}

#[cfg(windows)]
mod win {
    use super::WinToast;
    use windows::UI::Notifications::Management::{UserNotificationListener, UserNotificationListenerAccessStatus};
    use windows::UI::Notifications::NotificationKinds;
    use windows::Win32::System::WinRT::{RoInitialize, RO_INIT_MULTITHREADED};

    pub fn init_and_request_access() {
        // SAFETY: plain COM/WinRT initialisation of this thread; a repeated call just says so.
        unsafe {
            let _ = RoInitialize(RO_INIT_MULTITHREADED);
        }
        // Windows shows its own question the first time; the answer is read as the access status afterwards.
        if let Ok(listener) = UserNotificationListener::Current() {
            let _ = listener.RequestAccessAsync().and_then(|op| op.join());
        }
    }

    pub fn read_toasts() -> Result<Vec<WinToast>, String> {
        let listener = UserNotificationListener::Current().map_err(|e| e.message())?;
        if listener.GetAccessStatus().map_err(|e| e.message())? != UserNotificationListenerAccessStatus::Allowed {
            return Err("access to notifications is not allowed".to_string());
        }
        let list = listener.GetNotificationsAsync(NotificationKinds::Toast).and_then(|op| op.join()).map_err(|e| e.message())?;
        let mut out = Vec::new();
        for n in list {
            let info = n.AppInfo().map_err(|e| e.message())?;
            let app = info.DisplayInfo().and_then(|d| d.DisplayName()).map(|s| s.to_string()).unwrap_or_default();
            let aumid = info.AppUserModelId().map(|s| s.to_string()).unwrap_or_default();
            let mut texts = Vec::new();
            if let Ok(bindings) = n.Notification().and_then(|x| x.Visual()).and_then(|v| v.Bindings()) {
                for binding in bindings {
                    if binding.Template().map(|t| t.to_string()).unwrap_or_default() != "ToastGeneric" {
                        continue;
                    }
                    if let Ok(elements) = binding.GetTextElements() {
                        texts.extend(elements.into_iter().filter_map(|e| e.Text().ok()).map(|s| s.to_string()));
                    }
                }
            }
            out.push(WinToast { id: n.Id().map_err(|e| e.message())?, app, aumid, texts });
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIXTURE: &str = r#"<toast launch="x"><visual><binding template="ToastGeneric"><text>New message from Anna</text><text>See you at 6 &amp; bring the &quot;docs&quot;</text><text>Second line</text><text placement="attribution">via Mail</text><image src="a.png"/></binding></visual></toast>"#;

    #[test]
    fn texts_come_out_of_the_toast_generic_binding() {
        let texts = texts_from_toast_xml(FIXTURE);
        assert_eq!(texts, ["New message from Anna", "See you at 6 & bring the \"docs\"", "Second line"]);
        let (title, body) = title_and_body(&texts).unwrap();
        assert_eq!(title, "New message from Anna");
        assert_eq!(body.as_deref(), Some("See you at 6 & bring the \"docs\"\nSecond line"));
    }

    #[test]
    fn other_templates_and_empty_toasts_give_nothing() {
        assert!(texts_from_toast_xml("<toast><visual><binding template=\"ToastText02\"><text>x</text></binding></visual></toast>").is_empty());
        assert_eq!(title_and_body(&[]), None);
        assert_eq!(title_and_body(&["  ".to_string()]), None);
        assert_eq!(title_and_body(&["Only a title".to_string()]), Some(("Only a title".to_string(), None)));
    }

    #[test]
    fn texts_are_cut() {
        let long = "я".repeat(600);
        let (title, body) = title_and_body(&[long.clone(), long]).unwrap();
        assert_eq!((title.chars().count(), body.unwrap().chars().count()), (MAX_TITLE, MAX_BODY));
    }

    #[test]
    fn the_exception_list_ignores_case() {
        let exclude = vec!["Spotify".to_string(), "Steam".to_string()];
        assert!(is_excluded("spotify", &exclude));
        assert!(is_excluded("STEAM", &exclude));
        assert!(!is_excluded("Telegram", &exclude));
        assert!(!is_excluded("Spotify", &[]));
    }

    #[test]
    fn a_new_and_a_removed_toast_are_told_apart() {
        let toast = |id| WinToast { id, app: "A".into(), aumid: "A.B!C".into(), texts: vec!["t".into()] };
        let seen: HashSet<u32> = [1, 2, 3].into_iter().collect();
        let (added, removed) = diff(&seen, &[toast(2), toast(3), toast(4)]);
        assert_eq!(added.iter().map(|t| t.id).collect::<Vec<_>>(), [4]);
        assert_eq!(removed, [1]);
    }

    #[test]
    fn only_a_plain_aumid_is_opened() {
        assert!(valid_aumid("Microsoft.WindowsCalculator_8wekyb3d8bbwe!App"));
        assert!(valid_aumid("{1AC14E77-02E7-4E5D-B744-2EB1AE5198B7}\\notepad.exe"));
        assert!(!valid_aumid(""));
        assert!(!valid_aumid("a & calc.exe"));
        assert!(!valid_aumid("a\"b"));
        assert!(!valid_aumid(&"x".repeat(300)));
        let Some(Action::Launch { program, args, .. }) = open_action("A.B!C") else { panic!("an action") };
        assert_eq!((program.as_str(), args.unwrap()), ("explorer.exe", vec!["shell:AppsFolder\\A.B!C".to_string()]));
        assert!(open_action("bad;id").is_none());
    }

    #[test]
    fn the_app_list_is_kept_short() {
        let mut apps = BTreeSet::new();
        for i in 0..80 {
            remember_app(&mut apps, &format!("App {i:02}"));
        }
        assert_eq!(apps.len(), MAX_APPS);
        remember_app(&mut apps, "  ");
        assert_eq!(apps.len(), MAX_APPS);
    }
}
