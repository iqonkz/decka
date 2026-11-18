import { burnEta, formatLeft, locale, t, type AiItem, type AiProvider, type AiWindow } from "@decka/shared";
import { useDeckEnv } from "../deckEnv";
import { burnKey, useAiHistory, type AiHistory } from "./aiHistory";
import { LEVEL_COLOR } from "./statusOrder";

type Provider = "claude" | "codex";

/** The bar takes the accent; from 80% the `warning` colour, from 95% `error` (DATA_MODEL.md 6.8). */
export function barColor(pct: number): string {
  if (pct >= 95) return LEVEL_COLOR.error;
  if (pct >= 80) return LEVEL_COLOR.warning;
  return "var(--accent, #3b82f6)";
}

function WindowRow({ provider, w, now, showReset, showForecast, history }: { provider: Provider; w: AiWindow; now: number; showReset: boolean; showForecast: boolean; history: AiHistory }) {
  const left = w.resetsAt - now;
  const eta = showForecast ? burnEta(history[burnKey(provider, w.label)] ?? [], now, w.resetsAt) : null;
  const units = { d: t("ai.unit_d"), h: t("ai.unit_h"), m: t("ai.unit_m") };
  return (
    <div className="ai-window">
      <div className="ai-line">
        <span className="ai-label">{w.label}</span>
        <span className="ai-bar" role="progressbar" aria-valuemin={0} aria-valuemax={100} aria-valuenow={Math.round(w.pct)}>
          <span className="ai-fill" style={{ width: `${Math.min(100, Math.max(0, w.pct))}%`, background: barColor(w.pct) }} />
        </span>
        <span className="ai-pct">{Math.round(w.pct)}%</span>
        {showReset && left > 0 && <span className="ai-reset">{t("ai.resets_in", { time: formatLeft(left, units) })}</span>}
      </div>
      {eta !== null && (
        <div className="ai-forecast">{t("ai.runs_out", { time: new Intl.DateTimeFormat(locale(), { hour: "2-digit", minute: "2-digit" }).format(new Date(eta)) })}</div>
      )}
    </div>
  );
}

function ProviderBlock({ provider, data, windows, now, item, history }: { provider: Provider; data: AiProvider; windows: AiWindow[]; now: number; item: AiItem; history: AiHistory }) {
  return (
    <div className="ai-provider">
      <div className="ai-name">
        {t(`ai.${provider}`)}
        {data.plan !== undefined && <small className="ai-plan">{data.plan}</small>}
      </div>
      {windows.length === 0 && <div className="ai-line"><span className="ai-pct">—</span></div>}
      {windows.map((w) => (
        <WindowRow key={w.label} provider={provider} w={w} now={now} showReset={item.w >= 3} showForecast={item.w >= 3 && item.h >= 2} history={history} />
      ))}
    </div>
  );
}

/**
 * The AI limits widget (DATA_MODEL.md 6.8). 2x1: the short window of each provider; 2x2: both windows; 3x2: with the reset
 * times and the forecast; 4x2: the limits on the left and the Claude Code sessions on the right.
 */
export default function Ai({ item }: { item: AiItem }) {
  const { ai, onOpenPerm } = useDeckEnv();
  const history = useAiHistory();
  const show = item.show ?? "both";
  const now = Date.now();
  const compact = item.h < 2;
  const claude = ai?.claude ?? null;
  const codex = ai?.codex ?? null;
  const pending = ai?.pending.length ?? 0;
  const sessions = item.w >= 4 && item.h >= 2 ? (ai?.sessions ?? []) : [];
  const waiting = new Set((ai?.pending ?? []).map((p) => p.sessionId));
  // A widget of one provider has the room for both of its windows even at 2x1.
  const windowsOf = (p: AiProvider): AiWindow[] => (compact && show === "both" ? p.windows.slice(0, 1) : p.windows);

  return (
    <div className={compact ? "ai ai-compact" : "ai"}>
      <div className="ai-limits">
        {(show === "both" || show === "claude") &&
          (claude ? (
            <ProviderBlock provider="claude" data={claude} windows={windowsOf(claude)} now={now} item={item} history={history} />
          ) : (
            <div className="ai-nodata">
              <span>{t("ai.no_data")}</span>
              {!compact && <small>{t("ai.no_data_hint")}</small>}
            </div>
          ))}
        {(show === "both" || show === "codex") && codex && <ProviderBlock provider="codex" data={codex} windows={windowsOf(codex)} now={now} item={item} history={history} />}
      </div>
      {sessions.length > 0 && (
        <div className="ai-sessions">
          {sessions.map((s) => (
            <div key={s.id} className="ai-session">
              {waiting.has(s.id) && <span className="ai-wait-dot" style={{ background: LEVEL_COLOR.warning }} />}
              <span className="ai-project">{s.project}</span>
              <small className="ai-meta">
                {s.model ?? ""}
                {s.ctxPct !== undefined ? ` ${t("ai.ctx", { pct: s.ctxPct })}` : ""}
              </small>
            </div>
          ))}
        </div>
      )}
      {pending > 0 && (
        <button type="button" className="ai-badge" aria-label={t("ai.waiting", { n: pending })} onClick={() => onOpenPerm?.()}>
          {pending}
        </button>
      )}
    </div>
  );
}
