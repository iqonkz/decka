import { useCallback, useEffect, useState } from "react";
import type { ConfigFile, HttpCheck, ValidationIssue } from "@decka/shared";
import { call } from "../ipc";
import Banner from "../ui/Banner";
import Button from "../ui/Button";
import Field from "../ui/Field";
import { makeActionId } from "../editor/addButton";

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

const newCheck = (n: number): HttpCheck => ({ id: `check-${n}`, label: "", url: "https://", intervalSec: 60, timeoutSec: 5 });

/** The ID of a check is made from its name and never typed. */
const checkId = (label: string, existing: HttpCheck[]): string => makeActionId(label || "check", Object.fromEntries(existing.map((c) => [c.id, 1])));

export default function IntegrationsTab() {
  const [config, setConfig] = useState<ConfigFile | null>(null);
  const [port, setPort] = useState(9247);
  const [showToken, setShowToken] = useState(false);
  const [confirmRegen, setConfirmRegen] = useState(false);
  const [checks, setChecks] = useState<HttpCheck[]>([]);
  const [editing, setEditing] = useState<{ index: number; check: HttpCheck } | null>(null);
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

  async function saveChecks(next: HttpCheck[]) {
    const result = await call("save_config", { config: { ...config!, collectors: { ...config!.collectors, checks: next } } });
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

      <details className="advanced">
        <summary>Show examples</summary>
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
      </details>

      <section className="card">
        <h3>Website checks</h3>
        <p className="muted">The PC opens each address now and then and shows the result as a status on the phone.</p>
        {checks.length === 0 && <p className="muted">No checks yet.</p>}
        <ul className="plain-list">
          {checks.map((c, i) => (
            <li key={i} className="check-row">
              <span className="status-dot wait" aria-hidden="true" />
              <strong>{c.label}</strong>
              <span className="muted grow">{c.url}</span>
              <Button onClick={() => setEditing({ index: i, check: c })}>Edit</Button>
              <Button onClick={() => void saveChecks(checks.filter((_, n) => n !== i))}>Remove</Button>
            </li>
          ))}
        </ul>
        {issues.length > 0 && (
          <Banner kind="error">
            <ul>
              {issues.map((i, n) => (
                <li key={n}>{i.message}</li>
              ))}
            </ul>
          </Banner>
        )}
        {message && <Banner kind="success">{message}</Banner>}
        <div className="row">
          <Button variant="primary" disabled={checks.length >= 20} onClick={() => setEditing({ index: -1, check: newCheck(checks.length + 1) })}>
            Add check
          </Button>
        </div>
      </section>

      {editing && (
        <div className="dialog-backdrop">
          <div className="dialog" role="dialog" aria-label="Check">
            <h2>{editing.index < 0 ? "Add check" : "Edit check"}</h2>
            <Field label="Name">
              <input autoFocus value={editing.check.label} onChange={(e) => setEditing({ ...editing, check: { ...editing.check, label: e.target.value } })} />
            </Field>
            <Field label="Address">
              <input value={editing.check.url} onChange={(e) => setEditing({ ...editing, check: { ...editing.check, url: e.target.value } })} />
            </Field>
            <details className="advanced">
              <summary>More options</summary>
              <div className="grid-4">
                <Field label="Check every, s">
                  <input type="number" value={editing.check.intervalSec} onChange={(e) => setEditing({ ...editing, check: { ...editing.check, intervalSec: Number(e.target.value) } })} />
                </Field>
                <Field label="Give up after, s">
                  <input type="number" value={editing.check.timeoutSec} onChange={(e) => setEditing({ ...editing, check: { ...editing.check, timeoutSec: Number(e.target.value) } })} />
                </Field>
                <Field label="Expected answer" hint="Empty = any success">
                  <input
                    type="number"
                    placeholder="200"
                    value={editing.check.expectStatus ?? ""}
                    onChange={(e) => setEditing({ ...editing, check: { ...editing.check, expectStatus: e.target.value === "" ? undefined : Number(e.target.value) } })}
                  />
                </Field>
                <Field label="Group">
                  <input value={editing.check.group ?? ""} onChange={(e) => setEditing({ ...editing, check: { ...editing.check, group: e.target.value || undefined } })} />
                </Field>
              </div>
            </details>
            <div className="row">
              <Button
                variant="primary"
                onClick={() => {
                  const next = editing.index < 0 ? [...checks, { ...editing.check, id: checkId(editing.check.label, checks) }] : checks.map((c, n) => (n === editing.index ? editing.check : c));
                  setEditing(null);
                  void saveChecks(next);
                }}
              >
                Save
              </Button>
              <Button onClick={() => setEditing(null)}>Cancel</Button>
            </div>
          </div>
        </div>
      )}    </div>
  );
}
