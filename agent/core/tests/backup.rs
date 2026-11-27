//! The backup file (T31.4): from one PC to another and back, without secrets, with nothing written before `apply`.

use std::path::Path;
use std::sync::Arc;

use decka_core::actions::DryRunner;
use decka_core::agent::{Agent, AgentOptions};
use decka_core::backup;
use decka_core::model::{Action, ConfigFile};
use decka_core::SaveResult;
use serde_json::{json, Value};

const MAIL_PASSWORD: &str = "SECRET-MAIL-PASSWORD-44";
const OBS_PASSWORD: &str = "SECRET-OBS-PASSWORD-44";
const HEADER: &str = "SECRET-HEADER-VALUE-44";

async fn start(dir: &Path, config: &ConfigFile) -> Agent {
    decka_core::config::save_json_atomic(&dir.join("config.json"), config).unwrap();
    Agent::start(AgentOptions {
        bind_override: Some("127.0.0.1".to_string()),
        config_dir: Some(dir.to_path_buf()),
        client_dist: None,
        port_override: Some(0),
        runner: Arc::new(DryRunner),
        fixed_pair_code: None,
    })
    .await
    .unwrap()
}

fn quiet() -> ConfigFile {
    let mut config = decka_core::config::default_config();
    config.collectors.focus = Some(decka_core::model::SwitchConfig { enabled: false });
    config.collectors.media = Some(decka_core::model::SwitchConfig { enabled: false });
    config
}

/// A PC with a button of every kind that matters, secrets, a mode, a guide and dictation words.
async fn rich(dir: &Path) -> Agent {
    let mut config = quiet();
    config.agent_name = "Source".into();
    config.obs = Some(serde_json::from_value(json!({ "enabled": false, "port": 1997 })).unwrap());
    config.mail = Some(serde_json::from_value(json!({ "accounts": [{ "id": "m1", "name": "Home box", "host": "localhost", "port": 1, "user": "me@example.org" }] })).unwrap());
    let agent = start(dir, &config).await;
    agent.set_obs_password(OBS_PASSWORD).unwrap();
    agent.set_mail_password("m1", MAIL_PASSWORD).unwrap();
    let mut actions = agent.actions();
    actions.actions.insert("my-hook".into(), serde_json::from_value(json!({ "name": "My hook", "type": "http", "method": "POST", "url": "http://192.168.1.5/hook", "headers": { "Authorization": HEADER } })).unwrap());
    actions.actions.insert("my-editor".into(), serde_json::from_value(json!({ "name": "My editor", "type": "launch", "program": "C:\\Tools\\editor.exe" })).unwrap());
    assert_eq!(agent.save_actions(actions), SaveResult::Saved);
    let mut layout = agent.layout();
    let icon = decka_core::icons::icon_from_svg(dir, "<svg viewBox=\"0 0 8 8\"><path d=\"M0 0h8v8z\"/></svg>").unwrap();
    layout.pages[0].name = "Restored page".into();
    let mut with_icon = false;
    for item in &mut layout.pages[0].items {
        if let decka_core::model::LayoutItem::Button(button) = item {
            button.icon = Some(icon.clone());
            with_icon = true;
            break;
        }
    }
    assert!(with_icon);
    assert_eq!(agent.save_layout(layout), SaveResult::Saved);
    std::fs::write(dir.join("modes.json"), r#"[{"name":"Work","apps":[{"exe":"C:\\Tools\\mail.exe"}]}]"#).unwrap();
    decka_core::voice::guides::Guides::new(dir)
        .save("print a page", "winword.exe", vec![serde_json::from_value(json!({ "role": "button", "name": "Print" })).unwrap()], "2026-11-27")
        .unwrap();
    std::fs::write(dir.join("dictation.json"), r#"{"trigger":{"kind":"off"},"mode":"raw","dictionary":["Decka"]}"#).unwrap();
    std::fs::write(dir.join("voice.json"), r#"{"speak":true,"mic":"Source microphone","unloadAfterMin":30}"#).unwrap();
    agent
}

#[tokio::test(flavor = "multi_thread")]
async fn a_backup_restores_everything_but_the_secrets() {
    let (a_dir, b_dir, out) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let a = rich(a_dir.path()).await;
    let file = backup::export_to(&a, &out.path().join("mine")).unwrap();
    assert_eq!(file.extension().unwrap(), "decka");

    let secrets = a.config();
    let entries = decka_core::zipfile::read(&std::fs::read(&file).unwrap(), backup::MAX_BYTES).unwrap();
    let text: String = entries.iter().map(|(_, d)| String::from_utf8_lossy(d).to_string()).collect();
    for secret in [secrets.api_token.as_str(), MAIL_PASSWORD, OBS_PASSWORD, HEADER, "dpapi:"] {
        assert!(!text.contains(secret), "the backup holds {secret}");
    }

    let b = start(b_dir.path(), &quiet()).await;
    let b_before = (b.config(), b.actions(), b.layout());
    let preview = backup::preview(&b, &file).unwrap();
    // The preview wrote nothing.
    assert_eq!((b.config(), b.actions(), b.layout()), b_before);
    assert!(preview.actions.added.contains(&"My editor".to_string()), "{:?}", preview.actions);
    assert!(preview.pages.changed.contains(&"Restored page".to_string()), "{:?}", preview.pages);
    assert!(preview.runs.iter().any(|r| r == "C:\\Tools\\editor.exe") && preview.runs.iter().any(|r| r == "C:\\Tools\\mail.exe"), "{:?}", preview.runs);
    assert_eq!(preview.modes.added, ["Work"]);
    assert_eq!(preview.guides.added, ["print a page"]);
    for key in ["mail", "obs", "dictation", "voice"] {
        assert!(preview.settings.contains(&key.to_string()), "{key} in {:?}", preview.settings);
    }
    let kinds: Vec<&str> = preview.secrets.iter().map(|s| s.kind).collect();
    assert_eq!(kinds, ["mail", "obs", "webhook"]);

    assert!(backup::apply(&b, &file, "another digest").is_err());
    assert_eq!(b.actions(), b_before.1);

    let applied = backup::apply(&b, &file, &preview.digest).unwrap();
    assert!(applied.safety_copy.starts_with(b_dir.path().join("backups")) && applied.safety_copy.exists());
    let after = b.config();
    assert_eq!(after.api_token, b_before.0.api_token);
    assert_eq!(after.agent_name, b_before.0.agent_name);
    assert_eq!(after.mail.as_ref().unwrap().accounts[0].password, None);
    assert_eq!(after.obs.as_ref().unwrap().password, None);
    assert_eq!(b.layout().pages[0].name, "Restored page");
    let Action::Http { headers, .. } = &b.actions().actions["my-hook"] else { panic!("not a webhook") };
    assert_eq!(*headers, None);
    assert_eq!(decka_core::voice::modes::load(b_dir.path()).len(), 1);
    assert_eq!(decka_core::voice::guides::Guides::new(b_dir.path()).list().len(), 1);
    let voice: Value = serde_json::from_slice(&std::fs::read(b_dir.path().join("voice.json")).unwrap()).unwrap();
    // The microphone belongs to the PC.
    assert_eq!((voice["speak"].clone(), voice["unloadAfterMin"].clone(), voice["mic"].clone()), (json!(true), json!(30), Value::Null));
    assert!(std::fs::read_dir(b_dir.path().join("icons")).unwrap().flatten().any(|f| f.path().extension().is_some_and(|x| x == "svg")));

    // On the PC it came from, the secrets stay.
    let own = backup::preview(&a, &file).unwrap();
    assert!(own.secrets.is_empty() && own.settings.is_empty() && own.runs.is_empty(), "{own:?}");
    assert_eq!(own.actions, backup::Changes::default());
    backup::apply(&a, &file, &own.digest).unwrap();
    assert_eq!(a.config().mail.unwrap().accounts[0].password, secrets.mail.unwrap().accounts[0].password);
    assert_eq!(a.config().obs.unwrap().password, secrets.obs.unwrap().password);
    assert_eq!(a.actions().actions["my-hook"], { let Action::Http { .. } = &a.actions().actions["my-hook"] else { panic!() }; a.actions().actions["my-hook"].clone() });
    let Action::Http { headers: Some(headers), .. } = &a.actions().actions["my-hook"] else { panic!("the header is gone") };
    assert!(headers["Authorization"].starts_with("dpapi:"));
    a.shutdown().await;
    b.shutdown().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn a_broken_or_foreign_file_changes_nothing() {
    let (dir, out) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let agent = rich(dir.path()).await;
    let good = backup::export(&agent).unwrap();
    let before = (agent.config(), agent.actions(), agent.layout());
    let at = decka_core::assistant::Local::now();
    let entries = decka_core::zipfile::read(&good, backup::MAX_BYTES).unwrap();
    let with = |name: &str, content: &str| {
        let mut changed = entries.clone();
        match changed.iter_mut().find(|(n, _)| n == name) {
            Some(entry) => entry.1 = content.as_bytes().to_vec(),
            None => changed.push((name.to_string(), content.as_bytes().to_vec())),
        }
        decka_core::zipfile::write(&changed, &at).unwrap()
    };
    let cases: Vec<(&str, Vec<u8>)> = vec![
        ("not an archive", b"hello".to_vec()),
        ("another format", with("manifest.json", r#"{"format":"other","version":1,"decka":"1","made":"x"}"#)),
        ("a newer version", with("manifest.json", r#"{"format":"decka-backup","version":2,"decka":"9","made":"x"}"#)),
        ("a layout that points nowhere", with("actions.json", r#"{"version":1,"actions":{}}"#)),
        ("broken settings", with("config.json", "{")),
        ("a guide that leaves its folder", with("guides/x.json", r#"{"id":"../x","goal":["g"],"process":"a.exe","steps":[{"role":"button","name":"n"}],"created":"x"}"#)),
        ("an icon that is not what its name says", with("icons/0123456789abcdef.png", "text")),
    ];
    for (what, bytes) in cases {
        let file = out.path().join("bad.decka");
        std::fs::write(&file, &bytes).unwrap();
        let refused = backup::preview(&agent, &file);
        assert!(refused.is_err(), "{what} was accepted");
        assert!(backup::apply(&agent, &file, "x").is_err(), "{what} was applied");
    }
    assert_eq!((agent.config(), agent.actions(), agent.layout()), before);
    assert!(!dir.path().join("backups").exists());
    agent.shutdown().await;
}
