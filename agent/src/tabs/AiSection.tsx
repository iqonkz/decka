import { useEffect, useState } from "react";
import { locale, t, type AiConfig, type AiRule, type CcStatus, type ConfigFile } from "@decka/shared";
import { relTime } from "@decka/ui";
import { call } from "../ipc";
import Banner from "../ui/Banner";
import Button from "../ui/Button";
import Select from "../ui/Select";
import Switch from "../ui/Switch";

const DEFAULTS: AiConfig = { claude: { enabled: false, approvals: true, timeoutSec: 120 }, codex: { enabled: true } };
const WAITS = [60, 120, 300] as const;
/** The same three texts as `default_replies` in agent/core/src/ai.rs. */
const DEFAULT_REPLIES = ["Not this way: please try another approach.", "Show me your plan first and wait for my answer.", "Ask me before you change more files."];

/** The words of a rule of "Always allow" in the list. */
export function ruleText(r: AiRule): string {
  switch (r.kind) {
    case "command":
      return t("ai.rule_command", { value: r.value });
    case "project":
      return t("ai.rule_project", { value: r.value });
    case "host":
      return t("ai.rule_host", { value: r.value });
    default:
      return t("ai.rule_tool");
  }
}

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

  async function removeRule(id: string | undefined) {
    setError(null);
    try {
      await call("ai_remove_rule", id === undefined ? {} : { id });
      await onSaved();
    } catch (e) {
      setError(String(e));
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
      <label className="switch-row">
        <Switch checked={ai.claude.accountLimits === true} onChange={(checked) => void save({ ...ai, claude: { ...ai.claude, accountLimits: checked } })} label={t("aiset.account")} />
        <span>{t("aiset.account")}</span>
      </label>
      <small className="muted block">{t("aiset.account_hint")}</small>
      <label className="switch-row">
        <Switch checked={ai.claude.approvals} onChange={(checked) => void save({ ...ai, claude: { ...ai.claude, approvals: checked } })} label={t("aiset.approve")} />
        <span>{t("aiset.approve")}</span>
      </label>
      <div className="row">
        <label className="field-row">
          <span>{t("aiset.wait")}</span>
          <Select
            aria-label={t("aiset.wait")}
            value={String(ai.claude.timeoutSec)}
            disabled={!ai.claude.approvals}
            options={WAITS.map((n) => ({ value: String(n), label: t("aiset.seconds", { n }) }))}
            onChange={(val) => void save({ ...ai, claude: { ...ai.claude, timeoutSec: Number(val) as AiConfig["claude"]["timeoutSec"] } })}
          />
        </label>
      </div>
      <small className="muted block">{t("aiset.restart")}</small>

      <div className="row">
        <strong>{t("ai.codex")}</strong>
        <span className={status?.codexFound ? "ok-text" : "muted"}>{status?.codexFound ? t("aiset.codex_found") : t("aiset.codex_missing")}</span>
      </div>
      <label className="switch-row">
        <Switch checked={ai.codex.enabled} onChange={(checked) => void save({ ...ai, codex: { ...ai.codex, enabled: checked } })} label={t("aiset.codex_show")} />
        <span>{t("aiset.codex_show")}</span>
      </label>
      <label className="switch-row">
        <Switch
          checked={ai.codex.approvals !== false}
          disabled={!ai.codex.enabled}
          label={t("aiset.codex_approve")}
          onChange={(checked) => void save({ ...ai, codex: { ...ai.codex, approvals: checked } })}
        />
        <span>{t("aiset.codex_approve")}</span>
      </label>
      {installed && status?.codexHook && <small className="muted block">{t("aiset.codex_hook")}</small>}
      {!status?.codexHook && status?.codexFound && ai.codex.approvals !== false && <small className="muted block">{t("aiset.codex_install_hint")}</small>}

      <h4>{t("ai.replies_title")}</h4>
      <small className="muted block">{t("ai.replies_intro")}</small>
      <textarea
        key={(ai.replies ?? DEFAULT_REPLIES).join("\n")}
        rows={4}
        maxLength={500}
        defaultValue={(ai.replies ?? DEFAULT_REPLIES).join("\n")}
        onBlur={(e) => {
          const lines = e.target.value.split("\n").map((l) => l.trim().slice(0, 120)).filter((l) => l !== "").slice(0, 4);
          if (lines.join("\n") !== (ai.replies ?? DEFAULT_REPLIES).join("\n")) void save({ ...ai, replies: lines });
        }}
      />

      <h4>{t("ai.rules_title")}</h4>
      <small className="muted block">{t("ai.rules_intro")}</small>
      {(ai.rules ?? []).length === 0 ? (
        <small className="muted block">{t("ai.rules_none")}</small>
      ) : (
        <ul className="rules-list">
          {(ai.rules ?? []).map((r) => (
            <li key={r.id} className="row">
              <span>
                <strong>{t(r.provider === "codex" ? "ai.codex" : "ai.claude")}</strong> · {r.tool}
                {" · "}
                {ruleText(r)}
                {" · "}
                {/* Who made the rule and until when (DATA_MODEL.md 11.3): a phone's rule ends after 8 hours. */}
                <span className="muted">
                  {r.source !== undefined && r.expiresAt !== undefined
                    ? t("ai.rule_phone", { source: r.source, time: new Date(r.expiresAt).toLocaleTimeString(locale(), { hour: "2-digit", minute: "2-digit" }) })
                    : t("ai.rule_pc")}
                </span>
              </span>
              <span className="grow" />
              <Button onClick={() => void removeRule(r.id)}>{t("ai.rule_remove")}</Button>
            </li>
          ))}
        </ul>
      )}
      {(ai.rules ?? []).length > 1 && <Button onClick={() => void removeRule(undefined)}>{t("ai.rules_clear")}</Button>}
      {error && <Banner kind="error">{error}</Banner>}
    </section>
  );
}
