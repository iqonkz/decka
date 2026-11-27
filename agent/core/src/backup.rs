//! The backup file (T31.4): buttons, layout, work modes, guides and settings in one `.decka` (a ZIP), without secrets.
//!
//! Restoring is two steps. `preview` reads the file, validates everything and says what would change; `apply` does the same
//! again, refuses a file that is not the previewed one, writes a safety copy of the present state and only then replaces it.
//! Passwords and tokens never travel: the ones this PC already has stay where they belong, the others are listed to be
//! entered again. What guards this PC (the API token, the port and address, the allowed host names, the policy for public
//! networks, the "Always allow" rules, the path of adb) is never taken from the file.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::agent::Agent;
use crate::ai::service::{VoiceSettings, MAX_UNLOAD_MIN};
use crate::assistant::Local;
use crate::model::{Action, ActionsFile, ConfigFile, LayoutFile, ValidationIssue};
use crate::SaveResult;
use crate::voice::dictation::{self, DictationSettings, Trigger};
use crate::voice::guides::{Guides, Recipe};
use crate::voice::modes::{self, Mode};

const FORMAT: &str = "decka-backup";
const VERSION: u32 = 1;
/// A backup is a few hundred kilobytes; icons are what makes it grow.
pub const MAX_BYTES: usize = 40 * 1024 * 1024;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Manifest {
    format: String,
    version: u32,
    decka: String,
    made: String,
}

/// A secret that was in the settings of the backup and that this PC does not have.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub struct MissingSecret {
    /// `obs`, `mail`, `ha`, `twitch`, `youtube` or `webhook`.
    pub kind: &'static str,
    /// The mailbox, the address or the button it belongs to.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct Changes {
    pub added: Vec<String>,
    pub removed: Vec<String>,
    pub changed: Vec<String>,
}

/// What `Apply` would do.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Preview {
    /// The SHA-256 of the file: `apply` refuses another one.
    pub digest: String,
    pub made: String,
    pub decka_version: String,
    pub actions: Changes,
    pub pages: Changes,
    /// Names of the sections of the settings that change.
    pub settings: Vec<String>,
    /// Work modes and guides are added or replaced by name; the ones only this PC has stay.
    pub modes: Changes,
    pub guides: Changes,
    /// Programs and addresses that new or changed buttons, modes and checks start or call.
    pub runs: Vec<String>,
    pub secrets: Vec<MissingSecret>,
}

#[derive(Debug, Clone)]
pub struct Applied {
    /// The state before, kept in the config folder.
    pub safety_copy: PathBuf,
    pub secrets: Vec<MissingSecret>,
    /// Written to their files already; the running voice control takes them over too.
    pub voice: Option<VoiceSettings>,
    pub dictation: Option<DictationSettings>,
}

/// The content of a file, each part parsed.
struct Backup {
    manifest: Manifest,
    actions: ActionsFile,
    layout: LayoutFile,
    config: ConfigFile,
    modes: Vec<Mode>,
    guides: Vec<Recipe>,
    voice: Option<VoiceSettings>,
    dictation: Option<DictationSettings>,
    icons: Vec<(String, Vec<u8>)>,
}

/// Everything `apply` writes, validated.
struct Plan {
    actions: ActionsFile,
    layout: LayoutFile,
    config: ConfigFile,
    modes: Option<Vec<Mode>>,
    guides: Vec<Recipe>,
    voice: Option<VoiceSettings>,
    dictation: Option<DictationSettings>,
    icons: Vec<(String, Vec<u8>)>,
    preview: Preview,
}

// ---- Export ------------------------------------------------------------------------------------------------------

/// The settings without a secret in them; the `...Set` flags say which ones there were.
fn export_config(config: &ConfigFile) -> ConfigFile {
    let mut c = config.clone();
    c.api_token = String::new();
    c.server = None;
    if let Some(ai) = c.ai.as_mut() {
        ai.hook_token = None;
        ai.rules = Vec::new();
        ai.gpu_layers = None;
    }
    if let Some(obs) = c.obs.as_mut() {
        obs.password_set = Some(obs.password.is_some() || obs.password_set == Some(true));
        obs.password = None;
    }
    if let Some(mail) = c.mail.as_mut() {
        for a in &mut mail.accounts {
            a.password_set = Some(a.password.is_some() || a.password_set == Some(true));
            a.password = None;
        }
    }
    if let Some(yt) = c.youtube.as_mut() {
        yt.secret_set = Some(yt.secret.is_some() || yt.secret_set == Some(true));
        yt.tokens_set = Some(yt.tokens.is_some() || yt.tokens_set == Some(true));
        (yt.secret, yt.tokens, yt.channel) = (None, None, None);
    }
    if let Some(tw) = c.twitch.as_mut() {
        tw.tokens_set = Some(tw.tokens.is_some() || tw.tokens_set == Some(true));
        (tw.tokens, tw.login, tw.user_id) = (None, None, None);
    }
    if let Some(ha) = c.ha.as_mut() {
        ha.token = String::new();
    }
    c
}

/// Header values of webhooks are secrets: the names stay, the values go.
fn export_actions(actions: &ActionsFile) -> ActionsFile {
    let mut a = actions.clone();
    for action in a.actions.values_mut() {
        if let Action::Http { headers: Some(headers), .. } = action {
            headers.values_mut().for_each(String::clear);
        }
    }
    a
}

fn read_settings<T: serde::de::DeserializeOwned>(dir: &Path, name: &str) -> Option<T> {
    serde_json::from_str(&std::fs::read_to_string(dir.join(name)).ok()?).ok()
}

fn icon_files(dir: &Path, layout: &LayoutFile) -> Vec<(String, Vec<u8>)> {
    let icons = crate::icons::icons_dir(dir);
    crate::icons::referenced_hashes(layout)
        .into_iter()
        .filter_map(|hash| {
            ["png", "svg"].into_iter().find_map(|ext| {
                let name = format!("{hash}.{ext}");
                std::fs::read(icons.join(&name)).ok().map(|data| (name, data))
            })
        })
        .collect()
}

/// The backup of the present state.
pub fn export(agent: &Agent) -> Result<Vec<u8>, String> {
    let dir = agent.config_dir();
    let at = Local::now();
    let layout = agent.layout();
    let manifest = Manifest {
        format: FORMAT.into(),
        version: VERSION,
        decka: agent.status().version,
        made: format!("{:04}-{:02}-{:02} {:02}:{:02}", at.year, at.month, at.day, at.hour, at.minute),
    };
    fn json<T: Serialize>(name: &str, value: &T) -> Result<(String, Vec<u8>), String> {
        serde_json::to_vec_pretty(value).map(|v| (name.to_string(), v)).map_err(|e| e.to_string())
    }
    let mut files = vec![
        json("manifest.json", &manifest)?,
        json("actions.json", &export_actions(&agent.actions()))?,
        json("layout.json", &layout)?,
        json("config.json", &export_config(&agent.config()))?,
        json("modes.json", &modes::load(dir))?,
    ];
    if let Some(voice) = read_settings::<VoiceSettings>(dir, "voice.json") {
        files.push(json("voice.json", &voice)?);
    }
    if let Some(dictation) = read_settings::<DictationSettings>(dir, "dictation.json") {
        files.push(json("dictation.json", &dictation)?);
    }
    for recipe in Guides::new(dir).list() {
        // A guide whose id is not a safe file name was made by hand and stays here.
        if safe_id(&recipe.id) {
            files.push(json(&format!("guides/{}.json", recipe.id), &recipe)?);
        }
    }
    files.extend(icon_files(dir, &layout).into_iter().map(|(name, data)| (format!("icons/{name}"), data)));
    crate::zipfile::write(&files, &at).map_err(|e| e.to_string())
}

/// `export` into a file the user chose; `.decka` is added to a name without it.
pub fn export_to(agent: &Agent, path: &Path) -> Result<PathBuf, String> {
    let mut path = path.to_path_buf();
    if !path.extension().is_some_and(|e| e.eq_ignore_ascii_case("decka")) {
        let mut name = path.into_os_string();
        name.push(".decka");
        path = PathBuf::from(name);
    }
    std::fs::write(&path, export(agent)?).map_err(|e| format!("cannot write the backup: {e}"))?;
    Ok(path)
}

// ---- Read and check ----------------------------------------------------------------------------------------------

fn safe_id(id: &str) -> bool {
    !id.is_empty() && id.chars().count() <= 100 && id.chars().all(|c| c.is_alphanumeric() || c == '-' || c == '_')
}

fn parse<T: serde::de::DeserializeOwned>(name: &str, data: &[u8]) -> Result<T, String> {
    serde_json::from_slice(data).map_err(|e| format!("{name}: {e}"))
}

fn read(bytes: &[u8]) -> Result<Backup, String> {
    let files = crate::zipfile::read(bytes, MAX_BYTES).map_err(|e| e.to_string())?;
    let find = |name: &str| files.iter().find(|(n, _)| n == name).map(|(_, d)| d.as_slice());
    let need = |name: &str| find(name).ok_or_else(|| format!("{name} is not in the file"));
    let manifest: Manifest = parse("manifest.json", need("manifest.json")?)?;
    if manifest.format != FORMAT {
        return Err("This is not a Decka backup".into());
    }
    if manifest.version != VERSION {
        return Err("The backup was made by a newer Decka; update Decka first".into());
    }
    let mut guides = Vec::new();
    let mut icons = Vec::new();
    for (name, data) in &files {
        if let Some(file) = name.strip_prefix("guides/") {
            let recipe: Recipe = parse(name, data)?;
            let steps = recipe.steps.len();
            if !safe_id(&recipe.id) || file != format!("{}.json", recipe.id) || steps == 0 || steps > crate::voice::guides::MAX_RECIPE_STEPS {
                return Err(format!("{name}: not a guide"));
            }
            guides.push(recipe);
        } else if let Some(file) = name.strip_prefix("icons/") {
            // The name of an icon is the hash of its content, so a file cannot pose as another picture.
            let valid = file.split_once('.').is_some_and(|(hash, ext)| {
                crate::icons::is_hash(hash)
                    && crate::icons::hash_of(data) == hash
                    && match ext {
                        "png" => crate::icons::png_size(data).is_some() && crate::icons::data_url(data).len() <= crate::icons::MAX_DATA_URL_CHARS,
                        "svg" => std::str::from_utf8(data).is_ok_and(|svg| crate::icons::validate_svg(svg).is_ok()) && crate::icons::svg_data_url(data).len() <= crate::icons::MAX_SVG_DATA_URL_CHARS,
                        _ => false,
                    }
            });
            if !valid {
                return Err(format!("{name}: not an icon of Decka"));
            }
            icons.push((file.to_string(), data.clone()));
        }
    }
    Ok(Backup {
        actions: parse("actions.json", need("actions.json")?)?,
        layout: parse("layout.json", need("layout.json")?)?,
        config: parse("config.json", need("config.json")?)?,
        modes: find("modes.json").map(|d| parse("modes.json", d)).transpose()?.unwrap_or_default(),
        voice: find("voice.json").map(|d| parse("voice.json", d)).transpose()?,
        dictation: find("dictation.json").map(|d| parse("dictation.json", d)).transpose()?,
        guides,
        icons,
        manifest,
    })
}

// ---- Merge -------------------------------------------------------------------------------------------------------

/// The settings of the backup with what must stay from this PC, and the secrets that have to be entered again.
fn merge_config(current: &ConfigFile, backup: ConfigFile) -> (ConfigFile, Vec<MissingSecret>) {
    let mut missing = Vec::new();
    let mut need = |wanted: bool, have: bool, kind: &'static str, name: Option<String>| {
        if wanted && !have {
            missing.push(MissingSecret { kind, name });
        }
    };
    let mut c = backup;
    c.version = current.version;
    c.agent_name = current.agent_name.clone();
    c.port = current.port;
    c.bind = current.bind.clone();
    c.server = current.server.clone();
    c.network = current.network.clone();
    c.api_token = current.api_token.clone();
    c.usb.adb_path = current.usb.adb_path.clone();

    match c.ai.as_mut() {
        Some(ai) => {
            let own = current.ai.as_ref();
            ai.hook_token = own.and_then(|a| a.hook_token.clone());
            ai.rules = own.map(|a| a.rules.clone()).unwrap_or_default();
            ai.gpu_layers = own.and_then(|a| a.gpu_layers.clone());
        }
        // A section the backup does not have stays: dropping it would lose a secret for good.
        None => c.ai = current.ai.clone(),
    }
    match c.obs.as_mut() {
        Some(obs) => {
            let wanted = obs.password_set == Some(true);
            obs.password = current.obs.as_ref().and_then(|o| o.password.clone());
            obs.password_set = None;
            need(wanted, obs.password.is_some(), "obs", None);
        }
        None => c.obs = current.obs.clone(),
    }
    match c.mail.as_mut() {
        Some(mail) => {
            for a in &mut mail.accounts {
                let wanted = a.password_set == Some(true);
                // Only the same box on the same server: a password never goes to another host.
                a.password = current.mail.as_ref().and_then(|m| m.accounts.iter().find(|b| b.id == a.id && b.host == a.host && b.user == a.user)).and_then(|b| b.password.clone());
                a.password_set = None;
                need(wanted, a.password.is_some(), "mail", Some(a.name.clone()));
            }
        }
        None => c.mail = current.mail.clone(),
    }
    match c.youtube.as_mut() {
        Some(yt) => {
            let wanted = yt.secret_set == Some(true) || yt.tokens_set == Some(true);
            let own = current.youtube.as_ref().filter(|o| o.client_id == yt.client_id);
            (yt.secret, yt.tokens, yt.channel) = (own.and_then(|o| o.secret.clone()), own.and_then(|o| o.tokens.clone()), own.and_then(|o| o.channel.clone()));
            (yt.secret_set, yt.tokens_set) = (None, None);
            need(wanted, yt.tokens.is_some() || yt.secret.is_some(), "youtube", None);
        }
        None => c.youtube = current.youtube.clone(),
    }
    match c.twitch.as_mut() {
        Some(tw) => {
            let wanted = tw.tokens_set == Some(true);
            let own = current.twitch.as_ref().filter(|o| o.client_id == tw.client_id);
            (tw.tokens, tw.login, tw.user_id) = (own.and_then(|o| o.tokens.clone()), own.and_then(|o| o.login.clone()), own.and_then(|o| o.user_id.clone()));
            tw.tokens_set = None;
            need(wanted, tw.tokens.is_some(), "twitch", None);
        }
        None => c.twitch = current.twitch.clone(),
    }
    match c.ha.as_mut() {
        Some(ha) => match current.ha.as_ref().filter(|o| o.url == ha.url) {
            Some(own) => ha.token = own.token.clone(),
            // Without a token the section cannot be saved: this PC keeps its own, if it has one.
            None => {
                need(true, false, "ha", Some(ha.url.clone()));
                c.ha = current.ha.clone();
            }
        },
        None => c.ha = current.ha.clone(),
    }
    (c, missing)
}

/// A header value comes back only from the same button with the same address on this PC; else the header is dropped.
fn merge_actions(current: &ActionsFile, mut backup: ActionsFile) -> (ActionsFile, Vec<MissingSecret>) {
    let mut missing = Vec::new();
    for (id, action) in &mut backup.actions {
        let Action::Http { name, url, headers: Some(headers), .. } = action else { continue };
        let own = match current.actions.get(id) {
            Some(Action::Http { url: own_url, headers: Some(own), .. }) if own_url == url => Some(own),
            _ => None,
        };
        let mut lost = false;
        headers.retain(|header, value| {
            // A value never comes from the file.
            match own.and_then(|o| o.get(header)) {
                Some(kept) => {
                    *value = kept.clone();
                    true
                }
                None => {
                    lost = true;
                    false
                }
            }
        });
        if lost {
            missing.push(MissingSecret { kind: "webhook", name: Some(name.clone()) });
        }
        if headers.is_empty() {
            if let Action::Http { headers, .. } = action {
                *headers = None;
            }
        }
    }
    (backup, missing)
}

// ---- Compare -----------------------------------------------------------------------------------------------------

/// Added, removed and changed entries of two keyed lists; each entry is (display name, content).
fn changes(before: &BTreeMap<String, (String, Value)>, after: &BTreeMap<String, (String, Value)>) -> Changes {
    let mut out = Changes::default();
    for (key, (name, value)) in after {
        match before.get(key) {
            None => out.added.push(name.clone()),
            Some((_, old)) if old != value => out.changed.push(name.clone()),
            Some(_) => {}
        }
    }
    out.removed = before.iter().filter(|(key, _)| !after.contains_key(*key)).map(|(_, (name, _))| name.clone()).collect();
    out
}

fn value<T: Serialize>(v: &T) -> Value {
    serde_json::to_value(v).unwrap_or(Value::Null)
}

fn keyed_actions(a: &ActionsFile) -> BTreeMap<String, (String, Value)> {
    a.actions
        .iter()
        .map(|(id, action)| {
            let json = value(action);
            let name = json.get("name").and_then(Value::as_str).unwrap_or(id).to_string();
            (id.clone(), (name, json))
        })
        .collect()
}

fn keyed_pages(l: &LayoutFile) -> BTreeMap<String, (String, Value)> {
    l.pages.iter().map(|p| (p.id.clone(), (p.name.clone(), value(p)))).collect()
}

/// Every program and address inside a value.
fn collect_runs(v: &Value, out: &mut BTreeSet<String>) {
    match v {
        Value::Object(map) => {
            if let Some(program) = map.get("program").and_then(Value::as_str) {
                let args: Vec<&str> = map.get("args").and_then(Value::as_array).map(|a| a.iter().filter_map(Value::as_str).collect()).unwrap_or_default();
                out.insert(std::iter::once(program).chain(args).collect::<Vec<_>>().join(" "));
            }
            for key in ["url", "exe"] {
                if let Some(text) = map.get(key).and_then(Value::as_str) {
                    out.insert(text.to_string());
                }
            }
            map.values().for_each(|v| collect_runs(v, out));
        }
        Value::Array(items) => items.iter().for_each(|v| collect_runs(v, out)),
        _ => {}
    }
}

fn issues(file: &str, found: Vec<ValidationIssue>) -> Result<(), String> {
    match found.first() {
        None => Ok(()),
        Some(first) => Err(format!("{file}{}: {}", first.path, first.message)),
    }
}

/// The fields of `voice.json` that travel: the model, the microphone and the choice of the recognition belong to the PC.
fn merge_voice(current: &VoiceSettings, backup: VoiceSettings) -> VoiceSettings {
    VoiceSettings { model: current.model, mic: current.mic.clone(), light_stt: current.light_stt, light_stt_chosen: current.light_stt_chosen, ..backup }
}

fn plan(agent: &Agent, bytes: &[u8]) -> Result<Plan, String> {
    let backup = read(bytes)?;
    let dir = agent.config_dir();
    let (current_actions, current_layout, current_config) = (agent.actions(), agent.layout(), agent.config());

    issues("actions.json", crate::validate::validate_actions(&backup.actions))?;
    let (actions, mut secrets) = merge_actions(&current_actions, backup.actions);
    issues("actions.json", crate::validate::validate_actions(&actions))?;
    issues("layout.json", crate::validate::validate_layout(&backup.layout, &actions))?;
    let (config, missing) = merge_config(&current_config, backup.config);
    secrets.extend(missing);
    issues("config.json", crate::validate::validate_config(&config))?;
    secrets.sort();

    if let Some(bad) = backup.modes.iter().find(|m| m.name.trim().is_empty() || m.apps.len() > modes::MAX_APPS) {
        return Err(format!("modes.json: the mode '{}' is not valid", bad.name));
    }
    let current_modes = modes::load(dir);
    let mut merged_modes: Vec<Mode> = current_modes.clone();
    for mode in &backup.modes {
        match merged_modes.iter_mut().find(|m| m.name == mode.name) {
            Some(old) => *old = mode.clone(),
            None => merged_modes.push(mode.clone()),
        }
    }
    let keyed_modes = |list: &[Mode]| list.iter().map(|m| (m.name.clone(), (m.name.clone(), value(m)))).collect::<BTreeMap<_, _>>();
    let mode_changes = changes(&keyed_modes(&current_modes), &keyed_modes(&merged_modes));

    let current_guides = Guides::new(dir).list();
    let keyed_guides = |list: &[Recipe]| {
        list.iter()
            .map(|g| (g.id.clone(), (g.goal.first().cloned().unwrap_or_else(|| g.id.clone()), value(&(&g.goal, &g.process, &g.steps)))))
            .collect::<BTreeMap<_, _>>()
    };
    let mut after_guides = keyed_guides(&current_guides);
    after_guides.extend(keyed_guides(&backup.guides));
    let guide_changes = changes(&keyed_guides(&current_guides), &after_guides);

    let current_voice: VoiceSettings = read_settings(dir, "voice.json").unwrap_or_default();
    let voice = match backup.voice {
        Some(v) => {
            if v.unload_after_min > MAX_UNLOAD_MIN {
                return Err("voice.json: unloadAfterMin is out of range".into());
            }
            crate::voice::hotkey::parse_chord(&v.hotkey).map_err(|e| format!("voice.json: {e}"))?;
            Some(merge_voice(&current_voice, v)).filter(|merged| *merged != current_voice)
        }
        None => None,
    };
    let current_dictation = DictationSettings::load(dir);
    let dictation = match backup.dictation {
        Some(mut d) => {
            d.dictionary = dictation::normalize_dictionary(&d.dictionary);
            if let Trigger::Chord { keys } = &d.trigger {
                crate::voice::hotkey::parse_chord(keys).map_err(|e| format!("dictation.json: {e}"))?;
            }
            Some(d).filter(|d| *d != current_dictation)
        }
        None => None,
    };

    // The settings are compared without secrets, so that a kept password does not look like a change.
    let (before, after) = (value(&export_config(&current_config)), value(&export_config(&config)));
    let mut keys: BTreeSet<&String> = BTreeSet::new();
    keys.extend(before.as_object().into_iter().flat_map(|m| m.keys()));
    keys.extend(after.as_object().into_iter().flat_map(|m| m.keys()));
    let mut settings: Vec<String> = keys.into_iter().filter(|k| before.get(k.as_str()) != after.get(k.as_str())).cloned().collect();
    if current_layout.display != backup.layout.display {
        settings.push("display".into());
    }
    if voice.is_some() {
        settings.push("voice".into());
    }
    if dictation.is_some() {
        settings.push("dictation".into());
    }

    let action_changes = changes(&keyed_actions(&export_actions(&current_actions)), &keyed_actions(&export_actions(&actions)));
    // What the new state starts or calls that the present one does not.
    let mut had = BTreeSet::new();
    let mut has = BTreeSet::new();
    collect_runs(&value(&current_actions), &mut had);
    collect_runs(&value(&current_modes), &mut had);
    collect_runs(&value(&current_config.collectors.checks), &mut had);
    collect_runs(&value(&actions), &mut has);
    collect_runs(&value(&merged_modes), &mut has);
    collect_runs(&value(&config.collectors.checks), &mut has);

    let preview = Preview {
        digest: Sha256::digest(bytes).iter().map(|b| format!("{b:02x}")).collect(),
        made: backup.manifest.made.clone(),
        decka_version: backup.manifest.decka.clone(),
        actions: action_changes,
        pages: changes(&keyed_pages(&current_layout), &keyed_pages(&backup.layout)),
        settings,
        runs: has.difference(&had).cloned().collect(),
        secrets,
        modes: mode_changes,
        guides: guide_changes,
    };
    Ok(Plan {
        actions,
        layout: backup.layout,
        config,
        modes: (merged_modes != current_modes).then_some(merged_modes),
        guides: backup.guides,
        voice,
        dictation,
        icons: backup.icons,
        preview,
    })
}

fn load_file(path: &Path) -> Result<Vec<u8>, String> {
    let size = std::fs::metadata(path).map_err(|e| format!("cannot open the backup: {e}"))?.len();
    if size > MAX_BYTES as u64 {
        return Err("the archive is too large".into());
    }
    std::fs::read(path).map_err(|e| format!("cannot open the backup: {e}"))
}

/// What restoring `path` would change. Nothing is written.
pub fn preview(agent: &Agent, path: &Path) -> Result<Preview, String> {
    Ok(plan(agent, &load_file(path)?)?.preview)
}

/// Restores `path`, which must still be the previewed file.
pub fn apply(agent: &Agent, path: &Path, digest: &str) -> Result<Applied, String> {
    let plan = plan(agent, &load_file(path)?)?;
    if plan.preview.digest != digest {
        return Err("The file changed after it was checked; open it again".into());
    }
    let dir = agent.config_dir();
    let backups = dir.join("backups");
    std::fs::create_dir_all(&backups).map_err(|e| format!("cannot make the safety copy: {e}"))?;
    let at = Local::now();
    let stem = format!("before-restore-{:04}{:02}{:02}-{:02}{:02}", at.year, at.month, at.day, at.hour, at.minute);
    let safety_copy = crate::report::write_new(&backups, &stem, "decka", &export(agent)?)?;

    let failed = |what: &str, why: String| format!("{what} was not restored: {why}. The state before is in {}", safety_copy.display());
    let saved = |what: &str, result: SaveResult| match result {
        SaveResult::Saved => Ok(()),
        SaveResult::Rejected(found) => Err(failed(what, found.first().map(|i| i.message.clone()).unwrap_or_default())),
    };

    let icons = crate::icons::icons_dir(dir);
    std::fs::create_dir_all(&icons).map_err(|e| failed("icons", e.to_string()))?;
    for (name, data) in &plan.icons {
        let file = icons.join(name);
        if !file.exists() {
            std::fs::write(file, data).map_err(|e| failed("icons", e.to_string()))?;
        }
    }
    saved("actions.json", agent.save_actions(plan.actions))?;
    saved("layout.json", agent.save_layout(plan.layout))?;
    saved("config.json", agent.import_config(plan.config))?;
    if let Some(list) = &plan.modes {
        modes::write(dir, list).map_err(|e| failed("modes.json", e))?;
    }
    let guides = Guides::new(dir);
    for recipe in &plan.guides {
        guides.put(recipe).map_err(|e| failed("guides", e))?;
    }
    if let Some(voice) = &plan.voice {
        crate::config::save_json_atomic(&dir.join("voice.json"), voice).map_err(|e| failed("voice.json", e.to_string()))?;
    }
    if let Some(dictation) = &plan.dictation {
        dictation.save(dir).map_err(|e| failed("dictation.json", e))?;
    }
    Ok(Applied { safety_copy, secrets: plan.preview.secrets, voice: plan.voice, dictation: plan.dictation })
}
