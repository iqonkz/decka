//! The app strip (DATA_MODEL.md 10.5): for an app in front that has no page of its own, the keys of its built-in template.

use crate::model::{LayoutFile, LayoutItem, StripInfo, StripKey};
use crate::templates::PageTemplate;

/// The most keys a strip shows.
pub const MAX_STRIP_KEYS: usize = 16;

/// The strip for `focus_app`: `None` when no app is known, when a page of the layout already lists the app (the context page
/// of §6.1 applies), when no template names the app, or when the template has no keys. Keys are the buttons of the template
/// page in reading order (row by row), at most 16. With `lang` = `ru` the captions are the `nameRu` of the actions (and of the
/// template); a missing `nameRu` leaves the English one.
pub fn strip_for(focus_app: Option<&str>, layout: &LayoutFile, templates: &[PageTemplate], lang: &str) -> Option<StripInfo> {
    let app = focus_app?;
    if layout.pages.iter().any(|p| p.apps.as_ref().is_some_and(|apps| apps.iter().any(|a| a == app))) {
        return None;
    }
    let template = templates.iter().find(|t| t.apps.iter().any(|a| a == app))?;
    let mut buttons: Vec<_> = template
        .items
        .iter()
        .filter_map(|item| match item {
            LayoutItem::Button(b) if b.action_id.as_deref().is_some_and(|id| id.starts_with("tpl-")) => Some(b),
            _ => None,
        })
        .collect();
    buttons.sort_by_key(|b| (b.y, b.x));
    let keys: Vec<StripKey> = buttons
        .into_iter()
        .take(MAX_STRIP_KEYS)
        .map(|b| {
            let action_id = b.action_id.clone().unwrap_or_default();
            let russian = if lang == "ru" { template.action_names_ru.get(&action_id).cloned() } else { None };
            let label = russian.or_else(|| b.label.clone()).or_else(|| template.actions.get(&action_id).map(|a| a.name().to_string())).unwrap_or_default();
            StripKey { action_id, label, icon: b.icon.clone() }
        })
        .collect();
    if keys.is_empty() {
        return None;
    }
    let name = if lang == "ru" { template.name_ru.clone().unwrap_or_else(|| template.name.clone()) } else { template.name.clone() };
    Some(StripInfo { app: app.to_string(), template: template.id.clone(), name, keys, icon: None })
}

/// The program of `app` (a file name like `winword.exe`) among the installed ones, compared by file name without regard to case.
pub fn find_installed<'a>(installed: &'a [crate::installed::InstalledApp], app: &str) -> Option<&'a crate::installed::InstalledApp> {
    installed.iter().find(|i| i.program.rsplit(['\\', '/']).next().is_some_and(|file| file.eq_ignore_ascii_case(app)))
}

/// Puts the icon of the program on the strip when its exe is installed; `make_icon` turns a program path into `img:<hash16>`.
pub fn with_icon(mut strip: StripInfo, installed: &[crate::installed::InstalledApp], mut make_icon: impl FnMut(&str) -> Option<String>) -> StripInfo {
    strip.icon = find_installed(installed, &strip.app).and_then(|i| make_icon(&i.program));
    strip
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::default_layout;

    fn layout_with_page_for(app: Option<&str>) -> LayoutFile {
        let mut layout = default_layout();
        if let Some(app) = app {
            let mut page = layout.pages[0].clone();
            page.id = "own".into();
            page.apps = Some(vec![app.to_string()]);
            layout.pages.push(page);
        }
        layout
    }

    #[test]
    fn an_app_with_a_template_gets_its_keys() {
        let strip = strip_for(Some("winword.exe"), &layout_with_page_for(None), &crate::templates::all(), "en").expect("a strip");
        assert_eq!(strip.template, "word");
        assert_eq!(strip.app, "winword.exe");
        assert!(!strip.keys.is_empty() && strip.keys.len() <= MAX_STRIP_KEYS);
        assert!(strip.keys.iter().all(|k| k.action_id.starts_with("tpl-word-")));
        assert_eq!(strip.keys[0].action_id, "tpl-word-bold");
    }

    #[test]
    fn russian_captions_follow_the_language_and_fall_back_to_english() {
        let layout = layout_with_page_for(None);
        let templates = crate::templates::all();
        let ru = strip_for(Some("winword.exe"), &layout, &templates, "ru").unwrap();
        assert_eq!(ru.keys[0].action_id, "tpl-word-bold");
        assert_eq!(ru.keys[0].label, "Жирный");
        let en = strip_for(Some("winword.exe"), &layout, &templates, "en").unwrap();
        assert_eq!(en.keys[0].label, "Bold");
        // A template without `nameRu` keeps the English captions also in Russian.
        let mut plain = templates.clone();
        plain.iter_mut().for_each(|t| t.action_names_ru.clear());
        assert_eq!(strip_for(Some("winword.exe"), &layout, &plain, "ru").unwrap().keys[0].label, "Bold");
        // Every action of every template has a Russian name.
        for t in &templates {
            assert!(t.actions.keys().all(|id| t.action_names_ru.contains_key(id)), "{}", t.id);
        }
    }

    #[test]
    fn the_icon_is_set_when_the_exe_is_installed() {
        let app = |program: &str| crate::installed::InstalledApp { name: "x".into(), program: program.into(), args: vec![], cwd: None, icon: None };
        let strip = strip_for(Some("winword.exe"), &layout_with_page_for(None), &crate::templates::all(), "en").unwrap();
        let installed = vec![app(r"C:\Windows\notepad.exe"), app(r"C:\Program Files\Microsoft Office\root\Office16\WINWORD.EXE")];
        let with = with_icon(strip.clone(), &installed, |p| p.to_lowercase().ends_with("winword.exe").then(|| "img:0123456789abcdef".to_string()));
        assert_eq!(with.icon.as_deref(), Some("img:0123456789abcdef"));
        // Not installed (or no icon could be made): no field.
        assert_eq!(with_icon(strip.clone(), &installed[..1], |_| Some("img:0123456789abcdef".into())).icon, None);
        assert_eq!(with_icon(strip, &installed, |_| None).icon, None);
    }

    #[test]
    fn an_app_with_its_own_page_gets_none() {
        assert_eq!(strip_for(Some("winword.exe"), &layout_with_page_for(Some("winword.exe")), &crate::templates::all(), "en"), None);
    }

    #[test]
    fn no_template_or_no_app_gets_none() {
        let layout = layout_with_page_for(None);
        assert_eq!(strip_for(Some("notepad.exe"), &layout, &crate::templates::all(), "en"), None);
        assert_eq!(strip_for(None, &layout, &crate::templates::all(), "en"), None);
    }

    #[test]
    fn every_template_strip_has_at_most_16_keys() {
        let layout = layout_with_page_for(None);
        let templates = crate::templates::all();
        for t in &templates {
            for app in &t.apps {
                if let Some(strip) = strip_for(Some(app), &layout, &templates, "en") {
                    assert!(strip.keys.len() <= MAX_STRIP_KEYS, "{}", t.id);
                }
            }
        }
    }
}
