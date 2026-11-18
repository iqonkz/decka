import { locale, t, type PermLog } from "@decka/shared";
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

/** The list of the last questions of Claude Code / Codex and what was decided (a tap on the `ai` widget opens it). */
export default function PermHistory() {
  const open = useAppState((s) => s.permHistoryOpen);
  const history = useAppState((s) => s.state?.ai?.history);
  const waiting = useAppState((s) => (s.state?.ai?.pending.length ?? 0) > 0);
  if (!open || waiting) return null; // a question that waits for an answer comes first
  const time = new Intl.DateTimeFormat(locale(), { hour: "2-digit", minute: "2-digit" });
  return (
    <div className="perm-backdrop" onClick={closePermHistory}>
      <div className="perm-sheet" role="dialog" aria-label={t("hist.title")} onClick={(e) => e.stopPropagation()}>
        <h2 className="perm-title">{t("hist.title")}</h2>
        <div className="hist-list">
          {(history ?? []).length === 0 && <div className="perm-project">{t("hist.empty")}</div>}
          {(history ?? []).map((h, i) => (
            <div key={`${h.ts}-${i}`} className={`hist-row hist-${h.decision}`}>
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
        <button type="button" className="hist-close" onClick={closePermHistory}>
          {t("hist.close")}
        </button>
      </div>
    </div>
  );
}
