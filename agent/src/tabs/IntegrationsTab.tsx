import { useCallback, useEffect, useState } from "react";
import type { ConfigFile, HttpCheck, ValidationIssue } from "@decka/shared";
import { call } from "../ipc";
import Banner from "../ui/Banner";
import Button from "../ui/Button";

function CopyButton({ text }: { text: string }) {
  const [done, setDone] = useState(false);
  return (
    <Button
      onClick={() => {
        void navigator.clipboard.writeText(text).then(() => {
          setDone(true);
          window.setTimeout(() => setDone(false), 1500);
        });
      }}
    >
      {done ? "Copied" : "Copy"}
    </Button>
  );
}

function CommandBox({ title, command }: { title: string; command: string }) {
  return (
    <div className="command-box">
      <div className="row between">
        <strong>{title}</strong>
        <CopyButton text={command} />
      </div>
      <textarea className="mono" readOnly rows={command.split("\n").length} value={command} onFocus={(e) => e.currentTarget.select()} />
    </div>
  );
}

const newCheck = (n: number): HttpCheck => ({ id: `check-${n}`, label: "New check", url: "https://example.com", intervalSec: 60, timeoutSec: 5 });

export default function IntegrationsTab() {
  const [config, setConfig] = useState<ConfigFile | null>(null);
  const [port, setPort] = useState(9247);
  const [showToken, setShowToken] = useState(false);
  const [confirmRegen, setConfirmRegen] = useState(false);
  const [checks, setChecks] = useState<HttpCheck[]>([]);
  const [issues, setIssues] = useState<ValidationIssue[]>([]);
  const [message, setMessage] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  const load = useCallback(async () => {
    const c = await call("get_config");
    const s = await call("get_status");
    setConfig(c);
    setChecks(c.collectors.checks);
    setPort(s.listening?.port ?? c.port);
  }, []);

  useEffect(() => {
    load().catch((e) => setError(String(e)));
  }, [load]);

  if (error) return <Banner kind="error">{error}</Banner>;
  if (!config) return <p>Loading…</p>;

  const token = config.apiToken;
  const base = `http://127.0.0.1:${port}`;
  const auth = `-H "Authorization: Bearer ${token}"`;

  // JSON goes through a file: Windows PowerShell 5.1 mangles quotes in arguments of native programs.
  const curlJson = [
    `Set-Content -Path notify.json -Encoding ascii -Value '{"title":"Deploy finished","body":"web@1.4.2","level":"success","source":"ci"}'`,
    `curl.exe -s -X POST ${base}/api/notify ${auth} -H "Content-Type: application/json" --data-binary "@notify.json"`,
  ].join("\n");
  const curlText = `curl.exe -s -X POST ${base}/api/notify ${auth} -H "Content-Type: text/plain" -d "Backup finished"`;
  const powershell = `Invoke-RestMethod -Method Post -Uri ${base}/api/notify -Headers @{ Authorization = "Bearer ${token}" } -ContentType "application/json" -Body (@{ title = "Deploy finished"; level = "success" } | ConvertTo-Json)`;
  const curlStatus = [
    `Set-Content -Path status.json -Encoding ascii -Value '{"id":"my-job","label":"My job","state":"running","text":"working","group":"jobs","ttlSec":900}'`,
    `curl.exe -s -X POST ${base}/api/status ${auth} -H "Content-Type: application/json" --data-binary "@status.json"`,
  ].join("\n");

  async function regenerate() {
    setConfirmRegen(false);
    const result = await call("regenerate_api_token");
    setConfig({ ...config!, apiToken: result.apiToken });
  }

  const updateCheck = (i: number, patch: Partial<HttpCheck>) => {
    setChecks(checks.map((c, n) => (n === i ? { ...c, ...patch } : c)));
    setMessage(null);
  };

  async function saveChecks() {
    const result = await call("save_config", { config: { ...config!, collectors: { ...config!.collectors, checks } } });
    if (result.ok) {
      setIssues([]);
      setMessage("Saved");
      await load();
    } else {
      setIssues(result.issues);
      setMessage(null);
    }
  }

  return (
    <div className="stack">
      <h2>Integrations</h2>

      <section className="card">
        <h3>API token</h3>
        <div className="row">
          <input className="mono grow" readOnly type={showToken ? "text" : "password"} value={token} aria-label="API token" />
          <Button onClick={() => setShowToken(!showToken)}>{showToken ? "Hide" : "Show"}</Button>
          <CopyButton text={token} />
          <Button variant="danger" onClick={() => setConfirmRegen(true)}>
            Regenerate
          </Button>
        </div>
        {confirmRegen && (
          <Banner kind="warning">
            <span>Existing scripts will stop working.</span>
            <span className="row">
              <Button variant="danger" onClick={() => void regenerate()}>
                Regenerate
              </Button>
              <Button onClick={() => setConfirmRegen(false)}>Cancel</Button>
            </span>
          </Banner>
        )}
      </section>

      <section className="card">
        <h3>Send a notification</h3>
        <div className="stack">
          <CommandBox title="curl.exe (JSON)" command={curlJson} />
          <CommandBox title="curl.exe (plain text)" command={curlText} />
          <CommandBox title="PowerShell" command={powershell} />
          <div>
            <Button onClick={() => void call("send_test_notification")}>Send test notification</Button>
          </div>
        </div>
      </section>

      <section className="card">
        <h3>Update a status</h3>
        <CommandBox title="curl.exe" command={curlStatus} />
      </section>

      <section className="card">
        <h3>HTTP checks</h3>
        <p className="muted">The PC requests each URL and shows the result as a status on the phone.</p>
        {checks.length > 0 && (
          <table className="checks">
            <thead>
              <tr>
                <th>ID</th>
                <th>Label</th>
                <th>URL</th>
                <th>Every, s</th>
                <th>Timeout, s</th>
                <th>Expect</th>
                <th>Group</th>
                <th />
              </tr>
            </thead>
            <tbody>
              {checks.map((c, i) => (
                <tr key={i}>
                  <td><input aria-label="ID" value={c.id} onChange={(e) => updateCheck(i, { id: e.target.value })} /></td>
                  <td><input aria-label="Label" value={c.label} onChange={(e) => updateCheck(i, { label: e.target.value })} /></td>
                  <td><input aria-label="URL" value={c.url} onChange={(e) => updateCheck(i, { url: e.target.value })} /></td>
                  <td><input aria-label="Interval" type="number" value={c.intervalSec} onChange={(e) => updateCheck(i, { intervalSec: Number(e.target.value) })} /></td>
                  <td><input aria-label="Timeout" type="number" value={c.timeoutSec} onChange={(e) => updateCheck(i, { timeoutSec: Number(e.target.value) })} /></td>
                  <td>
                    <input
                      aria-label="Expected status"
                      type="number"
                      placeholder="2xx-3xx"
                      value={c.expectStatus ?? ""}
                      onChange={(e) => updateCheck(i, { expectStatus: e.target.value === "" ? undefined : Number(e.target.value) })}
                    />
                  </td>
                  <td><input aria-label="Group" value={c.group ?? ""} onChange={(e) => updateCheck(i, { group: e.target.value || undefined })} /></td>
                  <td>
                    <Button onClick={() => setChecks(checks.filter((_, n) => n !== i))}>Delete</Button>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        )}
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
        <div className="row">
          <Button disabled={checks.length >= 20} onClick={() => setChecks([...checks, newCheck(checks.length + 1)])}>
            Add check
          </Button>
          <Button variant="primary" onClick={() => void saveChecks()}>
            Save checks
          </Button>
        </div>
      </section>
    </div>
  );
}
