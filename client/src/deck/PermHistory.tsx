import { burnEta, locale, t, type AiInfo, type PermLog } from "@decka/shared";
import { burnKey, formatReset, levelColor, ProviderMark, useAiHistory, windowLabel } from "@decka/ui";
import { closePermHistory, useAppState } from "../store/store";

const MARK: Record<PermLog["decision"], string> = { allow: "✓", auto: "⚡", deny: "✗", pc: "↩" };

function decisionText(d: PermLog["decision"]): string {
  switch (d) {
    case "allow":
      return t("hist.allow");
    case "auto":
      return t("hist.auto");
    case "deny":
      return t("hist.deny");
    case "pc":
      return t("hist.pc");
  }
}

/** The details of the limits: exact percent and reset time of each window, the forecast, the plan, the data age, the sessions (DESIGN.md 4.11, 4.16). */
function Limits({ ai }: { ai: AiInfo }) {
  const history = useAiHistory();
  const now = Date.now();
  const hhmm = new Intl.DateTimeFormat(locale(), { hour: "2-digit", minute: "2-digit" });
  const units = { d: t("ai.unit_d"), h: t("ai.unit_h"), m: t("ai.unit_m") };
  const providers = (["claude", "codex"] as const).flatMap((p) => (ai[p] ? [{ id: p, data: ai[p] }] : []));
  if (providers.length === 0 && ai.sessions.length === 0) return <div className="ai-d-meta">{t("ai.no_data")}</div>;
  return (
    <div className="ai-detail">
      {providers.map(({ id, data }) =>
        data ? (
          <div key={id} className="phone-group ai-d-card">
            <div className="ai-d-prov">
              <div className="ai-d-head">
                <ProviderMark provider={id} size={18} />
                <span>{t(`ai.${id}`)}</span>
                {data.plan !== undefined && <span className="ai-d-plan">{t("ai.plan_label", { plan: data.plan })}</span>}
              </div>
              {data.windows.map((w) => {
                const eta = burnEta(history[burnKey(id, w.label)] ?? [], now, w.resetsAt);
                const left = w.resetsAt - now;
                return (
                  <div key={w.label}>
                    <div className="ai-d-win">
                      <span className="ai-label" style={{ width: 34 }}>{windowLabel(w.label, units)}</span>
                      <span className="ai-bar"><span className="ai-fill" style={{ width: `${Math.min(100, Math.max(0, w.pct))}%`, background: levelColor(w.pct) }} /></span>
                      <span className="ai-pct">{w.pct.toFixed(w.pct % 1 === 0 ? 0 : 1)}%</span>
                    </div>
                    {left > 0 && <div className="ai-d-sub">↻ {t("ai.resets_in", { time: formatReset(left, units) })}</div>}
                    {eta !== null && <div className="ai-d-sub warn">⚠ {t("ai.runs_out", { time: hhmm.format(new Date(eta)) })}</div>}
                  </div>
                );
              })}
              <div className="ai-d-meta">{t("ai.updated", { time: hhmm.format(new Date(data.updatedAt)) })}</div>
            </div>
          </div>
        ) : null,
      )}
      {ai.claude == null && <div className="ai-d-meta">{t("ai.no_data")} · {t("ai.no_data_hint")}</div>}
      {ai.sessions.length > 0 && (
        <div className="phone-group ai-d-card">
          <div className="ai-d-prov">
            <div className="ai-d-head">{t("ai.sessions_title")}</div>
            <div className="ai-d-sessions">
              {ai.sessions.map((s) => (
                <div key={s.id}>
                  <b>{s.project}</b>
                  <span className="ai-d-meta"> {s.model ?? ""}{s.ctxPct !== undefined ? ` · ${t("ai.ctx", { pct: s.ctxPct })}` : ""}</span>
                </div>
              ))}
            </div>
          </div>
        </div>
      )}
    </div>
  );
}

/** The sheet a tap on the `ai` widget opens (DESIGN.md 4.11, 4.16). */
export default function PermHistory() {
  const open = useAppState((s) => s.permHistoryOpen);
  const ai = useAppState((s) => s.state?.ai);
  const history = ai?.history;
  const waiting = useAppState((s) => (s.state?.ai?.pending.length ?? 0) > 0);
  if (!open || waiting) return null;
  const time = new Intl.DateTimeFormat(locale(), { hour: "2-digit", minute: "2-digit" });
  return (
    <div className="menu-backdrop" onPointerDown={closePermHistory}>
      <div className="menu-sheet perm-history-sheet" role="dialog" aria-label={t("ai.detail_title")} onPointerDown={(e) => e.stopPropagation()}>
        <div className="sheet-handle" aria-hidden="true" />
        <div className="sheet-scroll">
          <h2 className="sheet-title">{t("ai.detail_title")}</h2>
          {ai && <Limits ai={ai} />}
          <div className="phone-group-title">{t("hist.title")}</div>
          <div className="phone-group hist-list">
            {(history ?? []).length === 0 && <div className="phone-row"><span className="ai-d-meta">{t("hist.empty")}</span></div>}
            {(history ?? []).map((h, i) => (
              <div key={`${h.ts}-${i}`} className={`phone-row hist-row hist-${h.decision}`}>
                <span className="hist-mark" aria-label={decisionText(h.decision)}>{MARK[h.decision]}</span>
                <span className="hist-main">
                  <span className="hist-tool">
                    {t(`ai.${h.provider === "codex" ? "codex" : "claude"}`)} · {h.tool}
                    {h.project !== "" ? ` · ${h.project}` : ""}
                  </span>
                  <span className="hist-sum">{h.summary}</span>
                </span>
                <span className="hist-time">{time.format(new Date(h.ts))}</span>
              </div>
            ))}
          </div>
          <button type="button" className="pill-btn secondary" onClick={closePermHistory}>
            {t("hist.close")}
          </button>
        </div>
      </div>
    </div>
  );
}
