import { useCallback, useEffect, useState } from "react";
import type { ConfigFile, ValidationIssue } from "@decka/shared";
import { call } from "../ipc";
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

  const load = useCallback(async () => {
    setConfig(await call("get_config"));
    call("get_status").then((s) => setAdbMissing(!s.usb.adbFound), () => {});
  }, []);

  useEffect(() => {
    load().catch((e) => setError(String(e)));
  }, [load]);

  if (error) return <Banner kind="error">{error}</Banner>;
  if (!config) return <p>Loading…</p>;

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
    setMessage("Saved");
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
      <h2>Settings</h2>
      <section className="card">
        <Field label="PC name" hint="Shown on the phone.">
          <input value={config.agentName} maxLength={40} onChange={(e) => edit({ agentName: e.target.value })} />
        </Field>
        <label className="check">
          <input type="checkbox" checked={config.autostart} onChange={(e) => edit({ autostart: e.target.checked })} /> Start with Windows
        </label>
        <label className="check">
          <input type="checkbox" checked={config.usb.enabled} onChange={(e) => edit({ usb: { ...config.usb, enabled: e.target.checked } })} /> USB connection
        </label>
        <p className="muted">Needs USB debugging on the phone. Wi-Fi works without it.</p>
        {config.usb.enabled && adbMissing && (
          <Field label="adb program" hint="Decka could not find adb by itself: choose adb.exe from Android platform-tools.">
            <input value={config.usb.adbPath} onChange={(e) => edit({ usb: { ...config.usb, adbPath: e.target.value } })} />
          </Field>
        )}
      </section>

      <details className="advanced">
        <summary>More options</summary>
        <section className="card">
          <Field label="Port" hint="1024 to 65535. Applies after a server restart.">
            <input type="number" value={config.port} onChange={(e) => edit({ port: Number(e.target.value) })} />
          </Field>

          <fieldset className="radio-group">
            <legend>Who can connect</legend>
            <label>
              <input type="radio" name="bind" checked={config.bind === "0.0.0.0"} onChange={() => edit({ bind: "0.0.0.0" })} /> Phones on Wi-Fi and USB
            </label>
            <label>
              <input type="radio" name="bind" checked={config.bind === "127.0.0.1"} onChange={() => edit({ bind: "127.0.0.1" })} /> USB only
            </label>
          </fieldset>

          <div className="row">
            <label className="check">
              <input
                type="checkbox"
                checked={config.collectors.sys.enabled}
                onChange={(e) => edit({ collectors: { ...config.collectors, sys: { ...config.collectors.sys, enabled: e.target.checked } } })}
              />{" "}
              Show CPU and memory on the phone
            </label>
            <Field label="Refresh every, s">
              <input
                type="number"
                value={config.collectors.sys.intervalSec}
                onChange={(e) => edit({ collectors: { ...config.collectors, sys: { ...config.collectors.sys, intervalSec: Number(e.target.value) } } })}
              />
            </Field>
          </div>
          <Field label="adb program" hint="Only changes where Decka looks for adb.">
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
          <span>Restart the server to apply</span>
          <Button variant="primary" onClick={() => void restart()}>
            Restart now
          </Button>
        </Banner>
      )}

      <div className="row">
        <Button variant="primary" onClick={() => void save()}>
          Save
        </Button>
        <Button onClick={() => void load()}>Revert</Button>
      </div>
    </div>
  );
}
