import { useEffect, useMemo, useRef, useState } from "react";
import { t, type PermRequest } from "@decka/shared";
import { answerPerm } from "../app/controller";
import { hapticEvent } from "../platform/haptics";
import { openPerms, useAppState } from "../store/store";

/** `Allow` and `Deny` ignore touches for this long after a question appears (a stray touch must not decide it). */
export const PERM_GUARD_MS = 400;

/** A dangerous command is allowed by holding the button this long (DATA_MODEL.md 11.3). */
export const PERM_HOLD_MS = 1000;

/** `Always allow` is never offered for a dangerous command, nor for a whole tool: the PC does not accept those from a phone. */
export function offersAlways(request: PermRequest): boolean {
  return request.always !== undefined && request.always.kind !== "tool" && request.danger !== true;
}

/** `Allow` of a dangerous command: a hold with a ring that fills in a second; a tap only says `Hold to allow`. */
function HoldToAllow({ disabled, onHold }: { disabled: boolean; onHold: () => void }) {
  const [progress, setProgress] = useState(0);
  const [hint, setHint] = useState(false);
  const start = useRef<number | null>(null);
  const frame = useRef(0);

  const stop = () => {
    start.current = null;
    window.cancelAnimationFrame(frame.current);
    setProgress(0);
  };
  const step = () => {
    if (start.current === null) return;
    const p = Math.min(1, (Date.now() - start.current) / PERM_HOLD_MS);
    setProgress(p);
    if (p >= 1) {
      stop();
      onHold();
    } else {
      frame.current = window.requestAnimationFrame(step);
    }
  };
  useEffect(() => () => window.cancelAnimationFrame(frame.current), []);

  return (
    <button
      type="button"
      className="perm-allow perm-hold"
      disabled={disabled}
      onPointerDown={() => {
        setHint(false);
        start.current = Date.now();
        frame.current = window.requestAnimationFrame(step);
      }}
      onPointerUp={() => {
        if (start.current !== null) setHint(true); // released too early
        stop();
      }}
      onPointerLeave={stop}
      onPointerCancel={stop}
      onContextMenu={(e) => e.preventDefault()}
    >
      <svg className="perm-ring" viewBox="0 0 20 20" width="20" height="20" aria-hidden="true">
        <circle cx="10" cy="10" r="8" fill="none" stroke="currentColor" strokeOpacity="0.25" strokeWidth="2" />
        <circle cx="10" cy="10" r="8" fill="none" stroke="currentColor" strokeWidth="2" strokeDasharray={`${progress * 50.3} 50.3`} transform="rotate(-90 10 10)" />
      </svg>{" "}
      {hint ? t("perm.hold_hint") : t("perm.allow")}
    </button>
  );
}

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
    hapticEvent({ type: "confirm" }); // a short buzz: a question is waiting
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
  const title = t("perm.title", { name: t(`ai.${request.provider === "codex" ? "codex" : "claude"}`), tool: request.tool });
  const answer = (decision: "allow" | "allow_always" | "deny" | "pc" | "reply", reply?: number) => {
    hapticEvent({ type: "confirm" });
    answerPerm(request.reqId, decision, reply);
  };

  return (
    <div className="perm-backdrop">
      <div className="perm-sheet" role="dialog" aria-label={title}>
        {open.length > 1 && <div className="perm-count">{t("perm.count", { n: 1, total: open.length })}</div>}
        <h2 className="perm-title">{title}</h2>
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
          {request.danger === true ? (
            <HoldToAllow disabled={!armed} onHold={() => answer("allow")} />
          ) : (
            <button type="button" className="perm-allow" disabled={!armed} onClick={() => answer("allow")}>
              {t("perm.allow")}
            </button>
          )}
        </div>
        {request.always && offersAlways(request) && (
          <button type="button" className="perm-always" disabled={!armed} onClick={() => answer("allow_always")}>
            {t("perm.always", { what: request.always.value })}
          </button>
        )}
        {(request.replies ?? []).map((text, i) => (
          <button key={i} type="button" className="perm-reply" disabled={!armed} onClick={() => answer("reply", i)}>
            {t("perm.reply", { text })}
          </button>
        ))}
        <button type="button" className="perm-pc" onClick={() => answer("pc")}>
          {t("perm.pc")}
        </button>
      </div>
    </div>
  );
}
