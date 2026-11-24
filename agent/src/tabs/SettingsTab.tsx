import { useCallback, useEffect, useState } from "react";
import { t, type ConfigFile, type LanguageSetting, type ValidationIssue } from "@decka/shared";
import { call } from "../ipc";
import { saveLanguage } from "../language";
import LanguageOptions from "./LanguageOptions";
import Banner from "../ui/Banner";
import Button from "../ui/Button";
import { Group, PageHeader, Row, SectionHeader } from "../ui/SettingsGroup";
import Switch from "../ui/Switch";
import Segmented from "../ui/Segmented";

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
    <div className="page-container">
      <PageHeader title={t("more.settings")} />

      <Group>
        <Row id="settings-language" title={t("settings.language")}>
          <select
            className="settings-select"
            value={language}
            onChange={(e) => {
              const next = e.target.value as LanguageSetting;
              setLanguage(next);
              void saveLanguage(next);
            }}
          >
            <LanguageOptions />
          </select>
        </Row>
        <Row id="settings-pc-name" title={t("settings.pc_name")} description={t("settings.pc_name_hint")}>
          <input value={config.agentName} maxLength={40} onChange={(e) => edit({ agentName: e.target.value })} />
        </Row>
        <Row id="settings-autostart" title={t("settings.autostart")}>
          <Switch checked={config.autostart} onChange={(checked) => edit({ autostart: checked })} />
        </Row>
        <Row id="settings-usb" title={t("settings.usb")} description={t("settings.usb_hint")}>
          <Switch checked={config.usb.enabled} onChange={(checked) => edit({ usb: { ...config.usb, enabled: checked } })} />
        </Row>
        {config.usb.enabled && adbMissing && (
          <Row title={t("settings.adb")} description={t("settings.adb_missing_hint")}>
            <input value={config.usb.adbPath} onChange={(e) => edit({ usb: { ...config.usb, adbPath: e.target.value } })} />
          </Row>
        )}
      </Group>

      <details className="advanced" style={{ marginTop: 24 }}>
        <summary style={{ cursor: "pointer", fontWeight: 600 }}>{t("common.more_options")}</summary>
        <div id="settings-network" style={{ marginTop: 12 }}>
          <SectionHeader title={t("settings.port")} />
          <Group>
            <Row id="settings-port" title={t("settings.port")} description={t("settings.port_hint")}>
              <input type="number" value={config.port} onChange={(e) => edit({ port: Number(e.target.value) })} />
            </Row>
            <Row title={t("settings.who_connects")}>
              <Segmented
                options={[
                  { value: "0.0.0.0", label: t("settings.bind_all") },
                  { value: "127.0.0.1", label: t("settings.bind_usb") },
                ]}
                value={config.bind}
                onChange={(val) => edit({ bind: val })}
              />
            </Row>
            <Row title={t("settings.sys")}>
              <div className="row">
                <Switch
                  checked={config.collectors.sys.enabled}
                  onChange={(checked) => edit({ collectors: { ...config.collectors, sys: { ...config.collectors.sys, enabled: checked } } })}
                />
                <span className="muted">{t("settings.refresh")}</span>
                <input
                  type="number"
                  style={{ width: 80 }}
                  value={config.collectors.sys.intervalSec}
                  onChange={(e) => edit({ collectors: { ...config.collectors, sys: { ...config.collectors.sys, intervalSec: Number(e.target.value) } } })}
                />
              </div>
            </Row>
            <Row title={t("settings.adb")} description={t("settings.adb_hint")}>
              <input value={config.usb.adbPath} onChange={(e) => edit({ usb: { ...config.usb, adbPath: e.target.value } })} />
            </Row>
          </Group>
        </div>
      </details>

      {issues.length > 0 && (
        <div style={{ marginTop: 16 }}>
          <Banner kind="error">
            <ul>
              {issues.map((i, n) => (
                <li key={n}>
                  <code>{i.path || "/"}</code>: {i.message}
                </li>
              ))}
            </ul>
          </Banner>
        </div>
      )}
      {message && (
        <div style={{ marginTop: 16 }}>
          <Banner kind="success">{message}</Banner>
        </div>
      )}
      {needsRestart && (
        <div style={{ marginTop: 16 }}>
          <Banner kind="warning">
            <span>{t("settings.restart_needed")}</span>
            <Button variant="primary" onClick={() => void restart()}>
              {t("settings.restart_now")}
            </Button>
          </Banner>
        </div>
      )}

      <div className="row" style={{ marginTop: 24 }}>
        <Button variant="primary" onClick={() => void save()}>
          {t("common.save")}
        </Button>
        <Button onClick={() => void load()}>{t("common.revert")}</Button>
      </div>
    </div>
  );
}
