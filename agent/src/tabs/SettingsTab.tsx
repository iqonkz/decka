import { useCallback, useEffect, useState } from "react";
import { t, type ConfigFile, type LanguageSetting, type ValidationIssue } from "@decka/shared";
import { call } from "../ipc";
import { saveLanguage } from "../language";
import Banner from "../ui/Banner";
import Button from "../ui/Button";
import Field from "../ui/Field";

export default function SettingsTab() {
  const [config, setConfig] = useState<ConfigFile | null>(null);
  const [issues, setIssues] = useState<ValidationIssue[]>([]);
  const [message, setMessage] = useState<string | null>(null);
  const [needsRestart, setNeedsRestart] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [adbMissing, setAdbMissing] = useState(false);
  const [language, setLanguage] = useState<LanguageSetting>("auto");

  const load = useCallback(async () => {
    setConfig(await call("get_config"));
    call("get_status").then((s) => setAdbMissing(!s.usb.adbFound), () => {});
    call("get_layout").then((l) => setLanguage(l.display.language ?? "auto"), () => {});
  }, []);

  useEffect(() => {
    load().catch((e) => setError(String(e)));
  }, [load]);

  if (error) return <Banner kind="error">{error}</Banner>;
  if (!config) return <p>{t("common.loading")}</p>;

  const edit = (patch: Partial<ConfigFile>) => {
    setConfig({ ...config, ...patch });
    setMessage(null);
  };

  async function save() {
    const result = await call("save_config", { config: config! });
    if (!result.ok) {
      setIssues(result.issues);
      setMessage(null);
      return;
    }
    setIssues([]);
    setMessage(t("common.saved"));
    // Port and bind only apply after a restart: compare with what the server is really doing.
    const status = await call("get_status");
    setNeedsRestart(status.listening === null || status.listening.port !== config!.port || status.listening.bind !== config!.bind);
  }

  async function restart() {
    await call("restart_server");
    setNeedsRestart(false);
  }

  return (
    <div className="stack">
      <h2>{t("more.settings")}</h2>
      <section className="card">
        <Field label={t("settings.language")}>
          <select
            value={language}
            onChange={(e) => {
              const next = e.target.value as LanguageSetting;
              setLanguage(next);
              void saveLanguage(next);
            }}
          >
            <option value="auto">{t("settings.language_auto")}</option>
            <option value="en">English</option>
            <option value="ru">Русский</option>
          </select>
        </Field>
        <Field label={t("settings.pc_name")} hint={t("settings.pc_name_hint")}>
          <input value={config.agentName} maxLength={40} onChange={(e) => edit({ agentName: e.target.value })} />
        </Field>
        <label className="check">
          <input type="checkbox" checked={config.autostart} onChange={(e) => edit({ autostart: e.target.checked })} /> {t("settings.autostart")}
        </label>
        <label className="check">
          <input type="checkbox" checked={config.usb.enabled} onChange={(e) => edit({ usb: { ...config.usb, enabled: e.target.checked } })} /> {t("settings.usb")}
        </label>
        <p className="muted">{t("settings.usb_hint")}</p>
        {config.usb.enabled && adbMissing && (
          <Field label={t("settings.adb")} hint={t("settings.adb_missing_hint")}>
            <input value={config.usb.adbPath} onChange={(e) => edit({ usb: { ...config.usb, adbPath: e.target.value } })} />
          </Field>
        )}
      </section>

      <details className="advanced">
        <summary>{t("common.more_options")}</summary>
        <section className="card">
          <Field label={t("settings.port")} hint={t("settings.port_hint")}>
            <input type="number" value={config.port} onChange={(e) => edit({ port: Number(e.target.value) })} />
          </Field>

          <fieldset className="radio-group">
            <legend>{t("settings.who_connects")}</legend>
            <label>
              <input type="radio" name="bind" checked={config.bind === "0.0.0.0"} onChange={() => edit({ bind: "0.0.0.0" })} /> {t("settings.bind_all")}
            </label>
            <label>
              <input type="radio" name="bind" checked={config.bind === "127.0.0.1"} onChange={() => edit({ bind: "127.0.0.1" })} /> {t("settings.bind_usb")}
            </label>
          </fieldset>

          <div className="row">
            <label className="check">
              <input
                type="checkbox"
                checked={config.collectors.sys.enabled}
                onChange={(e) => edit({ collectors: { ...config.collectors, sys: { ...config.collectors.sys, enabled: e.target.checked } } })}
              />{" "}
              {t("settings.sys")}
            </label>
            <Field label={t("settings.refresh")}>
              <input
                type="number"
                value={config.collectors.sys.intervalSec}
                onChange={(e) => edit({ collectors: { ...config.collectors, sys: { ...config.collectors.sys, intervalSec: Number(e.target.value) } } })}
              />
            </Field>
          </div>
          <Field label={t("settings.adb")} hint={t("settings.adb_hint")}>
            <input value={config.usb.adbPath} onChange={(e) => edit({ usb: { ...config.usb, adbPath: e.target.value } })} />
          </Field>
        </section>
      </details>
      {issues.length > 0 && (
        <Banner kind="error">
          <ul>
            {issues.map((i, n) => (
              <li key={n}>
                <code>{i.path || "/"}</code>: {i.message}
              </li>
            ))}
          </ul>
        </Banner>
      )}
      {message && <Banner kind="success">{message}</Banner>}
      {needsRestart && (
        <Banner kind="warning">
          <span>{t("settings.restart_needed")}</span>
          <Button variant="primary" onClick={() => void restart()}>
            {t("settings.restart_now")}
          </Button>
        </Banner>
      )}

      <div className="row">
        <Button variant="primary" onClick={() => void save()}>
          {t("common.save")}
        </Button>
        <Button onClick={() => void load()}>{t("common.revert")}</Button>
      </div>
    </div>
  );
}
