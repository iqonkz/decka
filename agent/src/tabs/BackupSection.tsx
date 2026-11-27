// T31.4: save everything but the secrets into one `.decka` file, and restore it after showing what would change.
import { useState } from "react";
import { open, save } from "@tauri-apps/plugin-dialog";
import { t, type BackupChanges, type BackupPreview, type BackupRestored, type BackupSecret, type TranslationKey } from "@decka/shared";
import { call } from "../ipc";
import Banner from "../ui/Banner";
import Button from "../ui/Button";
import { Group, Row, SectionHeader } from "../ui/SettingsGroup";

const FILTERS = [{ name: "Decka", extensions: ["decka"] }];

const SETTING_NAMES: Record<string, TranslationKey> = {
  display: "backup.set_display",
  voice: "more.voice",
  dictation: "dict.title",
  mail: "mailset.title",
  weather: "weather.title",
  obs: "obsset.title",
  twitch: "twitchset.title",
  youtube: "youtubeset.title",
  media: "integ.item_lyrics_name",
  ai: "integ.item_ai_name",
  notes: "more.notes",
  windowsNotifications: "winnotif.title",
  collectors: "backup.set_collectors",
  island: "backup.set_island",
  autostart: "settings.autostart",
  updateCheck: "about.updates_toggle",
  usb: "settings.usb",
};

const SECRET_NAMES: Record<BackupSecret["kind"], TranslationKey> = {
  obs: "backup.secret_obs",
  mail: "backup.secret_mail",
  ha: "backup.secret_ha",
  twitch: "backup.secret_twitch",
  youtube: "backup.secret_youtube",
  webhook: "backup.secret_webhook",
};

/** A section without a name of its own (a product name such as `ha`) is shown as it is. */
const settingName = (key: string) => {
  const known = SETTING_NAMES[key];
  return known ? t(known) : key === "ha" ? "Home Assistant" : key;
};

const GROUPS = [
  { id: "actions", title: "backup.actions" },
  { id: "pages", title: "backup.pages" },
  { id: "modes", title: "backup.modes" },
  { id: "guides", title: "backup.guides" },
] as const;

const hasChanges = (c: BackupChanges) => c.added.length + c.removed.length + c.changed.length > 0;

function Secrets({ secrets }: { secrets: BackupSecret[] }) {
  if (secrets.length === 0) return null;
  return (
    <div>
      <strong className="block">{t("backup.secrets")}</strong>
      <ul>
        {secrets.map((s, i) => (
          <li key={i}>{t(SECRET_NAMES[s.kind], { name: s.name ?? "" })}</li>
        ))}
      </ul>
    </div>
  );
}

export function BackupChangesList({ preview }: { preview: BackupPreview }) {
  const nothing = preview.settings.length === 0 && GROUPS.every((g) => !hasChanges(preview[g.id]));
  return (
    <div className="backup-preview">
      <p>{t("backup.preview", { made: preview.made, version: preview.deckaVersion })}</p>
      {nothing && <p className="muted">{t("backup.nothing")}</p>}
      <ul>
        {GROUPS.filter((g) => hasChanges(preview[g.id])).map((g) => (
          <li key={g.id}>
            <strong>{t(g.title)}</strong>
            {(["added", "changed", "removed"] as const)
              .filter((kind) => preview[g.id][kind].length > 0)
              .map((kind) => (
                <span className="block" key={kind}>
                  {t(`backup.${kind}`, { list: preview[g.id][kind].join(", ") })}
                </span>
              ))}
          </li>
        ))}
        {preview.settings.length > 0 && (
          <li>
            <strong>{t("backup.settings")}</strong>
            <span className="block">{preview.settings.map(settingName).join(", ")}</span>
          </li>
        )}
      </ul>
      {preview.runs.length > 0 && (
        <Banner kind="warning">
          <strong className="block">{t("backup.runs")}</strong>
          <ul>
            {preview.runs.map((r) => (
              <li key={r}>
                <code>{r}</code>
              </li>
            ))}
          </ul>
          <span className="block">{t("backup.runs_warn")}</span>
        </Banner>
      )}
      <Secrets secrets={preview.secrets} />
    </div>
  );
}

export default function BackupSection({ onRestored }: { onRestored: () => Promise<void> }) {
  const [busy, setBusy] = useState(false);
  const [saved, setSaved] = useState<string | null>(null);
  const [checked, setChecked] = useState<{ path: string; preview: BackupPreview } | null>(null);
  const [restored, setRestored] = useState<BackupRestored | null>(null);
  const [error, setError] = useState<string | null>(null);

  async function run(work: () => Promise<void>) {
    setBusy(true);
    setError(null);
    try {
      await work();
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  }

  const clear = () => {
    setSaved(null);
    setChecked(null);
    setRestored(null);
  };

  const exportFile = () =>
    run(async () => {
      const path = await save({ defaultPath: "decka.decka", filters: FILTERS });
      if (!path) return;
      clear();
      setSaved(await call("backup_export", { path }));
    });

  const openFile = () =>
    run(async () => {
      const path = await open({ multiple: false, directory: false, filters: FILTERS });
      if (typeof path !== "string") return;
      clear();
      setChecked({ path, preview: await call("backup_preview", { path }) });
    });

  const apply = () =>
    run(async () => {
      if (!checked) return;
      const result = await call("backup_apply", { path: checked.path, digest: checked.preview.digest });
      setChecked(null);
      setRestored(result);
      window.dispatchEvent(new CustomEvent("decka:restored"));
      await onRestored();
    });

  return (
    <>
      <SectionHeader id="section-settings-backup" title={t("backup.title")} />
      <Group>
        <Row title={t("backup.row")} description={t("backup.hint")}>
          <div className="row">
            <Button disabled={busy} onClick={() => void exportFile()}>
              {t("backup.save")}
            </Button>
            <Button disabled={busy} onClick={() => void openFile()}>
              {t("backup.restore")}
            </Button>
          </div>
        </Row>
      </Group>
      {saved && <p className="ok-text" role="status">{t("backup.saved", { path: saved })}</p>}
      {checked && (
        <div className="card" style={{ marginTop: 12 }}>
          <BackupChangesList preview={checked.preview} />
          <div className="row" style={{ marginTop: 12 }}>
            <Button variant="primary" disabled={busy} onClick={() => void apply()}>
              {t("backup.apply")}
            </Button>
            <Button disabled={busy} onClick={clear}>
              {t("common.cancel")}
            </Button>
          </div>
        </div>
      )}
      {restored && (
        <Banner kind="success">
          <span className="block">{t("backup.restored", { path: restored.safetyCopy })}</span>
          <Secrets secrets={restored.secrets} />
          {restored.voiceError && <span className="block">{t("backup.voice_error", { error: restored.voiceError })}</span>}
        </Banner>
      )}
      {error && <Banner kind="error">{error}</Banner>}
    </>
  );
}
