//! Installed programs for the "Choose installed app" picker (M12, DATA_MODEL.md 8.3): Start menu shortcuts to `.exe` files.
//! Microsoft Store apps without an `.exe` shortcut are not listed.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

/// Longest list the window gets.
pub const MAX_APPS: usize = 500;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstalledApp {
    /// Shortcut name without `.lnk`.
    pub name: String,
    /// Full path of the `.exe`.
    pub program: String,
    pub args: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cwd: Option<String>,
    /// 32 px PNG data URL for the list; may be absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
}

/// A shortcut as found on disk, before filtering.
#[derive(Debug, Clone)]
pub struct Found {
    pub app: InstalledApp,
    /// From the user's Start menu (`%APPDATA%`) rather than the shared one (`%ProgramData%`).
    pub per_user: bool,
}

/// Uninstall shortcuts are not programs to launch: by shortcut name (`uninstall`, `удал`) or target (`unins*.exe`).
pub fn is_uninstaller(name: &str, target: &str) -> bool {
    let name = name.to_lowercase();
    if name.contains("uninstall") || name.contains("удал") {
        return true;
    }
    let file = target.rsplit(['\\', '/']).next().unwrap_or(target).to_lowercase();
    file.starts_with("unins")
}

/// One entry per (target path in lower case, arguments); between a shared and a per-user shortcut the per-user one stays.
pub fn dedupe(found: Vec<Found>) -> Vec<InstalledApp> {
    let mut by_key: HashMap<(String, Vec<String>), Found> = HashMap::new();
    let mut order: Vec<(String, Vec<String>)> = Vec::new();
    for f in found {
        let key = (f.app.program.to_lowercase(), f.app.args.clone());
        match by_key.get(&key) {
            None => {
                order.push(key.clone());
                by_key.insert(key, f);
            }
            Some(existing) if f.per_user && !existing.per_user => {
                by_key.insert(key, f);
            }
            Some(_) => {}
        }
    }
    let mut apps: Vec<InstalledApp> = order.into_iter().filter_map(|k| by_key.remove(&k)).map(|f| f.app).collect();
    sort_by_name(&mut apps);
    apps
}

/// Alphabetical, ignoring case.
pub fn sort_by_name(apps: &mut [InstalledApp]) {
    apps.sort_by_cached_key(|a| (a.name.to_lowercase(), a.program.to_lowercase()));
}

/// Start menu programs; slow (seconds): call it off the UI thread. Other platforms: empty.
pub fn list_installed_apps() -> Vec<InstalledApp> {
    #[cfg(windows)]
    {
        crate::sta::run(win::scan).unwrap_or_default()
    }
    #[cfg(not(windows))]
    {
        Vec::new()
    }
}

/// File name without the extension: the name of a program that has no better one.
pub fn stem_of(path: &str) -> String {
    let file = path.rsplit(['\\', '/']).next().unwrap_or(path);
    match file.rfind('.') {
        Some(i) if i > 0 => file[..i].to_string(),
        _ => file.to_string(),
    }
}

/// What the picker would show for one file dropped or chosen by the user (PROTOCOL.md 9, `describe_program`):
/// `.exe` -> the file's `FileDescription` (else its name without `.exe`); `.lnk` -> the shortcut's target, arguments and
/// working folder. Only reads the file; nothing is started.
pub fn describe_program(path: &str) -> Result<InstalledApp, String> {
    let lower = path.to_lowercase();
    let is_exe = lower.ends_with(".exe");
    let is_lnk = lower.ends_with(".lnk");
    if !is_exe && !is_lnk {
        return Err("Only .exe programs and .lnk shortcuts are supported".to_string());
    }
    if !std::path::Path::new(path).is_file() {
        return Err("The file does not exist".to_string());
    }
    #[cfg(windows)]
    {
        let path = path.to_string();
        crate::sta::run(move || if is_lnk { win::describe_lnk(&path) } else { win::describe_exe(&path) })?
    }
    #[cfg(not(windows))]
    {
        Err("Reading programs is only supported on Windows".to_string())
    }
}
#[cfg(windows)]
mod win {
    use std::collections::HashMap;
    use std::path::{Path, PathBuf};

    use windows::core::{Interface, HSTRING, PCWSTR};
    use windows::Win32::Foundation::{HLOCAL, MAX_PATH};
    use windows::Win32::Foundation::LocalFree;
    use windows::Win32::Storage::FileSystem::WIN32_FIND_DATAW;
    use windows::Win32::System::Com::{CoCreateInstance, IPersistFile, CLSCTX_INPROC_SERVER, STGM_READ};
    use windows::Win32::UI::Shell::{CommandLineToArgvW, IShellLinkW, ShellLink};

    use super::{dedupe, is_uninstaller, Found, InstalledApp, MAX_APPS};

    const START_MENU: &str = r"Microsoft\Windows\Start Menu\Programs";

    fn roots() -> Vec<(PathBuf, bool)> {
        let mut roots = Vec::new();
        if let Some(p) = std::env::var_os("ProgramData") {
            roots.push((Path::new(&p).join(START_MENU), false));
        }
        if let Some(p) = std::env::var_os("APPDATA") {
            roots.push((Path::new(&p).join(START_MENU), true));
        }
        roots
    }

    fn collect_lnk(dir: &Path, out: &mut Vec<PathBuf>, depth: usize) {
        let Ok(entries) = std::fs::read_dir(dir) else { return };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                if depth < 8 {
                    collect_lnk(&path, out, depth + 1);
                }
            } else if path.extension().is_some_and(|e| e.eq_ignore_ascii_case("lnk")) {
                out.push(path);
            }
        }
    }

    fn wide_to_string(buf: &[u16]) -> String {
        let end = buf.iter().position(|c| *c == 0).unwrap_or(buf.len());
        String::from_utf16_lossy(&buf[..end])
    }

    /// Splits a command line by the Windows rules (`CommandLineToArgvW`).
    fn split_args(args: &str) -> Vec<String> {
        if args.trim().is_empty() {
            return Vec::new();
        }
        // The API treats the first token as the program name, so a dummy one goes in front.
        let line = HSTRING::from(format!("x {args}"));
        let mut count = 0i32;
        // SAFETY: the returned block is read within `count` entries and freed once.
        unsafe {
            let argv = CommandLineToArgvW(PCWSTR(line.as_ptr()), &mut count);
            if argv.is_null() {
                return Vec::new();
            }
            let out = (1..count.max(1) as usize).filter_map(|i| (*argv.add(i)).to_string().ok()).collect();
            let _ = LocalFree(Some(HLOCAL(argv as *mut _)));
            out
        }
    }

    /// (target, arguments, working folder) of a shortcut.
    fn read_link(link: &IShellLinkW, file: &Path) -> Option<(String, String, String)> {
        // SAFETY: COM calls on the apartment thread with buffers that outlive them.
        unsafe {
            let persist: IPersistFile = link.cast().ok()?;
            persist.Load(&HSTRING::from(file.as_os_str()), STGM_READ).ok()?;
            let mut target = [0u16; MAX_PATH as usize];
            let mut find = WIN32_FIND_DATAW::default();
            link.GetPath(&mut target, &mut find, 0).ok()?;
            let mut args = [0u16; 2048];
            link.GetArguments(&mut args).ok()?;
            let mut cwd = [0u16; MAX_PATH as usize];
            link.GetWorkingDirectory(&mut cwd).ok()?;
            Some((wide_to_string(&target), wide_to_string(&args), wide_to_string(&cwd)))
        }
    }

    /// `FileDescription` from the version resource of an `.exe`, if it has one.
    fn file_description(path: &str) -> Option<String> {
        use windows::Win32::Storage::FileSystem::{GetFileVersionInfoSizeW, GetFileVersionInfoW, VerQueryValueW};
        let file = HSTRING::from(path);
        // SAFETY: the buffer is sized by the API itself and outlives the queries; returned pointers point into it.
        unsafe {
            let size = GetFileVersionInfoSizeW(PCWSTR(file.as_ptr()), None);
            if size == 0 {
                return None;
            }
            let mut block = vec![0u8; size as usize];
            GetFileVersionInfoW(PCWSTR(file.as_ptr()), None, size, block.as_mut_ptr().cast()).ok()?;
            let mut ptr: *mut std::ffi::c_void = std::ptr::null_mut();
            let mut len = 0u32;
            // The first (language, code page) pair of the file; English and neutral as fallbacks.
            let mut langs: Vec<String> = Vec::new();
            if VerQueryValueW(block.as_ptr().cast(), &HSTRING::from(r"\VarFileInfo\Translation"), &mut ptr, &mut len).as_bool() && len >= 4 {
                let pair = std::slice::from_raw_parts(ptr as *const u16, 2);
                langs.push(format!("{:04x}{:04x}", pair[0], pair[1]));
            }
            langs.push("040904b0".to_string());
            langs.push("040904e4".to_string());
            langs.push("000004b0".to_string());
            for lang in langs {
                let key = HSTRING::from(format!(r"\StringFileInfo\{lang}\FileDescription"));
                if VerQueryValueW(block.as_ptr().cast(), &key, &mut ptr, &mut len).as_bool() && len > 1 {
                    let text = std::slice::from_raw_parts(ptr as *const u16, len as usize);
                    let text = wide_to_string(text).trim().to_string();
                    if !text.is_empty() {
                        return Some(text);
                    }
                }
            }
            None
        }
    }

    pub fn describe_exe(path: &str) -> Result<InstalledApp, String> {
        let name = file_description(path).unwrap_or_else(|| super::stem_of(path));
        Ok(InstalledApp { name, program: path.to_string(), args: Vec::new(), cwd: None, icon: crate::icons::win::list_icon_data_url(path) })
    }

    pub fn describe_lnk(path: &str) -> Result<InstalledApp, String> {
        // SAFETY: standard in-process COM activation on the apartment thread.
        let link: IShellLinkW = unsafe { CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER) }.map_err(|e| format!("Cannot read the shortcut: {e}"))?;
        let file = Path::new(path);
        let (target, args, cwd) = read_link(&link, file).ok_or_else(|| "Cannot read the shortcut".to_string())?;
        if !target.to_lowercase().ends_with(".exe") || !Path::new(&target).is_file() {
            return Err("The shortcut does not point to a program (.exe)".to_string());
        }
        let name = super::stem_of(path);
        let cwd = (!cwd.is_empty()).then_some(cwd);
        let icon = crate::icons::win::list_icon_data_url(&target);
        Ok(InstalledApp { name, program: target, args: split_args(&args), cwd, icon })
    }

    /// Creates a shortcut file (tests).
    #[cfg(test)]
    pub fn write_lnk(lnk: &str, target: &str, args: &str) -> Result<(), String> {
        let link: IShellLinkW = unsafe { CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER) }.map_err(|e| e.to_string())?;
        unsafe {
            link.SetPath(&HSTRING::from(target)).map_err(|e| e.to_string())?;
            link.SetArguments(&HSTRING::from(args)).map_err(|e| e.to_string())?;
            let persist: IPersistFile = link.cast().map_err(|e| e.to_string())?;
            persist.Save(&HSTRING::from(lnk), true).map_err(|e| e.to_string())
        }
    }
    pub fn scan() -> Vec<InstalledApp> {
        // SAFETY: standard in-process COM activation on the apartment thread.
        let link: IShellLinkW = match unsafe { CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER) } {
            Ok(link) => link,
            Err(e) => {
                tracing::warn!("cannot read shortcuts: {e}");
                return Vec::new();
            }
        };
        let mut found = Vec::new();
        for (root, per_user) in roots() {
            let mut files = Vec::new();
            collect_lnk(&root, &mut files, 0);
            for file in files {
                let Some(name) = file.file_stem().map(|s| s.to_string_lossy().into_owned()) else { continue };
                let Some((target, args, cwd)) = read_link(&link, &file) else { continue };
                if !target.to_lowercase().ends_with(".exe") || !Path::new(&target).is_file() || is_uninstaller(&name, &target) {
                    continue;
                }
                let cwd = (!cwd.is_empty()).then_some(cwd);
                found.push(Found { app: InstalledApp { name, program: target, args: split_args(&args), cwd, icon: None }, per_user });
            }
        }
        let mut apps = dedupe(found);
        apps.truncate(MAX_APPS);
        // One Shell icon per program, however many shortcuts point at it.
        let mut icons: HashMap<String, Option<String>> = HashMap::new();
        for app in &mut apps {
            app.icon = icons.entry(app.program.to_lowercase()).or_insert_with(|| crate::icons::win::list_icon_data_url(&app.program)).clone();
        }
        apps
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn app(name: &str, program: &str, args: &[&str]) -> InstalledApp {
        InstalledApp { name: name.into(), program: program.into(), args: args.iter().map(|a| a.to_string()).collect(), cwd: None, icon: None }
    }

    #[test]
    fn uninstallers_are_recognised_by_name_and_target() {
        assert!(is_uninstaller("Uninstall Foo", r"C:\Foo\foo.exe"));
        assert!(is_uninstaller("Удалить Foo", r"C:\Foo\foo.exe"));
        assert!(is_uninstaller("Remove Foo", r"C:\Foo\unins000.exe"));
        assert!(is_uninstaller("Foo", r"C:\Foo\UNINS000.EXE"));
        assert!(!is_uninstaller("Foo", r"C:\Foo\foo.exe"));
        assert!(!is_uninstaller("Union", r"C:\Foo\union.exe"));
    }

    #[test]
    fn a_duplicate_keeps_the_per_user_shortcut() {
        let shared = Found { app: app("Foo (shared)", r"C:\Foo\Foo.exe", &[]), per_user: false };
        let mine = Found { app: app("Foo", r"c:\foo\foo.exe", &[]), per_user: true };
        let apps = dedupe(vec![shared.clone(), mine.clone()]);
        assert_eq!(apps, vec![mine.app.clone()]);
        // The order of discovery does not matter.
        assert_eq!(dedupe(vec![mine.clone(), shared]), vec![mine.app]);
    }

    #[test]
    fn different_arguments_are_different_entries() {
        let a = Found { app: app("Foo", r"C:\Foo\foo.exe", &[]), per_user: false };
        let b = Found { app: app("Foo safe mode", r"C:\Foo\foo.exe", &["--safe"]), per_user: false };
        assert_eq!(dedupe(vec![a, b]).len(), 2);
    }

    #[test]
    fn the_list_is_sorted_ignoring_case() {
        let found = ["zeta", "Alpha", "beta", "Gamma"].iter().map(|n| Found { app: app(n, &format!(r"C:\{n}.exe"), &[]), per_user: false }).collect();
        let names: Vec<String> = dedupe(found).into_iter().map(|a| a.name).collect();
        assert_eq!(names, ["Alpha", "beta", "Gamma", "zeta"]);
    }

    #[cfg(windows)]
    #[test]
    fn the_real_start_menu_lists_existing_exe_programs() {
        let apps = list_installed_apps();
        assert!(apps.len() <= MAX_APPS);
        for a in &apps {
            assert!(a.program.to_lowercase().ends_with(".exe"), "{}", a.program);
            assert!(std::path::Path::new(&a.program).exists(), "{}", a.program);
            assert!(!is_uninstaller(&a.name, &a.program), "{}", a.name);
        }
        let names: Vec<String> = apps.iter().map(|a| a.name.to_lowercase()).collect();
        let mut sorted = names.clone();
        sorted.sort();
        assert_eq!(names, sorted);
    }

    #[test]
    fn stem_drops_the_directory_and_the_extension() {
        assert_eq!(stem_of(r"C:\Apps\Foo Bar.exe"), "Foo Bar");
        assert_eq!(stem_of("foo.exe"), "foo");
        assert_eq!(stem_of(r"C:\x\.hidden"), ".hidden");
    }

    #[test]
    fn other_extensions_and_missing_files_are_errors() {
        assert!(describe_program(r"C:\Windows\System32\drivers\etc\hosts").is_err());
        assert!(describe_program(r"C:\Windows\readme.txt").unwrap_err().contains(".exe"));
        assert_eq!(describe_program(r"C:\definitely\not\here.exe").unwrap_err(), "The file does not exist");
        assert_eq!(describe_program(r"C:\definitely\not\here.lnk").unwrap_err(), "The file does not exist");
    }

    #[cfg(windows)]
    #[test]
    fn an_exe_is_named_by_its_file_description() {
        let notepad = r"C:\Windows\System32\notepad.exe";
        let app = describe_program(notepad).unwrap();
        assert_eq!(app.program, notepad);
        assert!(app.args.is_empty() && app.cwd.is_none());
        // The description is localised ("Notepad", "Блокнот"), so only its presence is checked.
        assert!(!app.name.trim().is_empty());
        assert!(app.icon.as_deref().is_some_and(|i| i.starts_with("data:image/png;base64,")));
    }

    #[cfg(windows)]
    #[test]
    fn a_shortcut_gives_its_target_arguments_and_name() {
        let dir = tempfile::tempdir().unwrap();
        let lnk = dir.path().join("My Notes.lnk");
        let notepad = r"C:\Windows\System32\notepad.exe";
        let (l, n) = (lnk.to_string_lossy().into_owned(), notepad.to_string());
        crate::sta::run(move || win::write_lnk(&l, &n, r#"--flag "two words""#)).unwrap().unwrap();
        let app = describe_program(&lnk.to_string_lossy()).unwrap();
        assert_eq!(app.name, "My Notes");
        assert_eq!(app.program.to_lowercase(), notepad.to_lowercase());
        assert_eq!(app.args, ["--flag", "two words"]);
    }

    #[cfg(windows)]
    #[test]
    fn a_shortcut_to_a_non_program_is_an_error() {
        let dir = tempfile::tempdir().unwrap();
        let lnk = dir.path().join("Hosts.lnk");
        let (l, n) = (lnk.to_string_lossy().into_owned(), r"C:\Windows\System32\drivers\etc\hosts".to_string());
        crate::sta::run(move || win::write_lnk(&l, &n, "")).unwrap().unwrap();
        assert!(describe_program(&lnk.to_string_lossy()).is_err());
    }}
