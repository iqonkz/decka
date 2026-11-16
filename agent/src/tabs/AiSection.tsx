import { useEffect, useState } from "react";
import { t, type AiConfig, type CcStatus, type ConfigFile } from "@decka/shared";
import { relTime } from "@decka/ui";
import { call } from "../ipc";
import Banner from "../ui/Banner";
import Button from "../ui/Button";

const DEFAULTS: AiConfig = { claude: { enabled: false, approvals: true, timeoutSec: 120 }, codex: { enabled: true } };
const WAITS = [60, 120, 300] as const;

/** `More → Integrations → AI coding` (DATA_MODEL.md 6.8): Claude Code is installed into its own settings and removed again; Codex is only read. */
export default function AiSection({ config, onSaved }: { config: ConfigFile; onSaved: () => Promise<void> }) {
  const ai = config.ai ?? DEFAULTS;
  const [status, setStatus] = useState<CcStatus | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  // What Claude Code did last is asked for every 2 seconds while this page is open.
  useEffect(() => {
    let live = true;
    const ask = () => void call("cc_status").then((s) => live && setStatus(s), () => live && setStatus(null));
    ask();
    const timer = window.setInterval(ask, 2000);
    return () => {
      live = false;
      window.clearInterval(timer);
    };
  }, []);

  async function run(command: "cc_install" | "cc_uninstall") {
    setBusy(true);
    setError(null);
    try {
      setStatus(await call(command));
      await onSaved();
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  }

  async function save(next: AiConfig) {
    setError(null);
    const fresh = await call("get_config");
    const result = await call("save_config", { config: { ...fresh, ai: next } });
    if (!result.ok) return setError(result.issues[0]?.message ?? "");
    await onSaved();
  }

  const installed = status?.installed === true;
  let state: string;
  if (!installed) state = t("aiset.not_installed");
  else if (status?.lastSeenAt === undefined) state = t("aiset.waiting");
  else state = t("aiset.connected", { ago: relTime(status.lastSeenAt, Date.now()) });

  return (
    <section className="card" id="ai">
      <h3>{t("aiset.title")}</h3>

      <div className="row">
        <strong>{t("aiset.claude")}</strong>
        <span className={installed ? "ok-text" : "muted"}>{state}</span>
        <span className="grow" />
        {installed ? (
          <Button disabled={busy} onClick={() => void run("cc_uninstall")}>{t("aiset.remove")}</Button>
        ) : (
          <Button variant="primary" disabled={busy} onClick={() => void run("cc_install")}>{t("aiset.install")}</Button>
        )}
      </div>
      {installed && status?.statusLineConflict && <Banner kind="warning">{t("aiset.conflict")}</Banner>}
      <label className="check">
        <input type="checkbox" checked={ai.claude.approvals} onChange={(e) => void save({ ...ai, claude: { ...ai.claude, approvals: e.target.checked } })} /> {t("aiset.approve")}
      </label>
      <div className="row">
        <label className="field-row">
          <span>{t("aiset.wait")}</span>
          <select value={ai.claude.timeoutSec} disabled={!ai.claude.approvals} onChange={(e) => void save({ ...ai, claude: { ...ai.claude, timeoutSec: Number(e.target.value) as AiConfig["claude"]["timeoutSec"] } })}>
            {WAITS.map((n) => (
              <option key={n} value={n}>{t("aiset.seconds", { n })}</option>
            ))}
          </select>
        </label>
      </div>
      <small className="muted block">{t("aiset.restart")}</small>

      <div className="row">
        <strong>Codex</strong>
        <span className={status?.codexFound ? "ok-text" : "muted"}>{status?.codexFound ? t("aiset.codex_found") : t("aiset.codex_missing")}</span>
      </div>
      <label className="check">
        <input type="checkbox" checked={ai.codex.enabled} onChange={(e) => void save({ ...ai, codex: { enabled: e.target.checked } })} /> {t("aiset.codex_show")}
      </label>
      <small className="muted block">{t("aiset.codex_note")}</small>
      {error && <Banner kind="error">{error}</Banner>}
    </section>
  );
}
