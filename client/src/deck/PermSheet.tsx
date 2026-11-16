import { useEffect, useMemo, useState } from "react";
import { t } from "@decka/shared";
import { answerPerm } from "../app/controller";
import { hapticEvent } from "../platform/haptics";
import { openPerms, useAppState } from "../store/store";

/** `Allow` and `Deny` ignore touches for this long after a question appears (a stray touch must not decide it). */
export const PERM_GUARD_MS = 400;

/**
 * The permission sheet (DATA_MODEL.md 6.8): over any page while Claude Code waits for an answer. It closes at once when answered
 * here, and quietly when the question left `pending` (answered on the PC, or the wait ran out).
 */
export default function PermSheet() {
  const pending = useAppState((s) => s.state?.ai?.pending);
  const closed = useAppState((s) => s.permClosed);
  const open = useMemo(() => openPerms(pending, closed), [pending, closed]);
  const request = open[0];
  const reqId = request?.reqId;

  const [armed, setArmed] = useState(false);
  const [expanded, setExpanded] = useState(false);
  const [now, setNow] = useState(() => Date.now());
  const [shownAt, setShownAt] = useState(() => Date.now());
  useEffect(() => {
    setArmed(false);
    setExpanded(false);
    setShownAt(Date.now());
    if (!reqId) return;
    const guard = window.setTimeout(() => setArmed(true), PERM_GUARD_MS);
    const tick = window.setInterval(() => setNow(Date.now()), 250);
    return () => {
      window.clearTimeout(guard);
      window.clearInterval(tick);
    };
  }, [reqId]);

  if (!request) return null;
  const total = Math.max(1, request.expiresAt - shownAt);
  const left = Math.min(1, Math.max(0, (request.expiresAt - now) / total));
  const answer = (decision: "allow" | "deny" | "pc") => {
    hapticEvent({ type: "confirm" });
    answerPerm(request.reqId, decision);
  };

  return (
    <div className="perm-backdrop">
      <div className="perm-sheet" role="dialog" aria-label={t("perm.title", { tool: request.tool })}>
        {open.length > 1 && <div className="perm-count">{t("perm.count", { n: 1, total: open.length })}</div>}
        <h2 className="perm-title">{t("perm.title", { tool: request.tool })}</h2>
        {request.project !== "" && <div className="perm-project">{request.project}</div>}
        {request.summary !== "" && (
          <button type="button" className={expanded ? "perm-summary expanded" : "perm-summary"} aria-label={t("perm.expand")} onClick={() => setExpanded((e) => !e)}>
            {request.summary}
          </button>
        )}
        <div className="perm-timer" aria-hidden="true">
          <span style={{ width: `${left * 100}%` }} />
        </div>
        <div className="perm-actions">
          <button type="button" className="perm-deny" disabled={!armed} onClick={() => answer("deny")}>
            {t("perm.deny")}
          </button>
          <button type="button" className="perm-allow" disabled={!armed} onClick={() => answer("allow")}>
            {t("perm.allow")}
          </button>
        </div>
        <button type="button" className="perm-pc" onClick={() => answer("pc")}>
          {t("perm.pc")}
        </button>
      </div>
    </div>
  );
}
