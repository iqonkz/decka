import type { ReactNode } from "react";
import { burnEta, locale, t, type AiItem, type AiProvider, type AiWindow } from "@decka/shared";
import { useDeckEnv } from "../deckEnv";
import { useDeckMetrics } from "../metricsContext";
import { ProviderMark } from "./aiMarks";
import { aiMode, closestWindow, formatReset, levelOf, MIN_TEXT, ringGeometry, rowsFit, textWidth, windowLabel, type Level } from "./aiModel";
import { burnKey, useAiHistory, type AiHistory } from "./aiHistory";
import { LEVEL_COLOR } from "./statusOrder";

type Provider = "claude" | "codex";

const PAD = 2;
const LEVEL_FILL: Record<Level, string> = { accent: "var(--accent, #3b82f6)", warning: LEVEL_COLOR.warning, error: LEVEL_COLOR.error };

/** Colour of a ring or bar by level (DESIGN.md 4.11: 70 / 90). */
export function levelColor(pct: number): string {
  return LEVEL_FILL[levelOf(pct)];
}

interface Entry {
  provider: Provider;
  data: AiProvider | null;
}

/** What a provider says about its closest window: `↻ 2h 14m`, or `⚠ 14:20` when the limit ends before the reset. */
function resetLine(provider: Provider, data: AiProvider, now: number, history: AiHistory): { text: string; warn: boolean } | null {
  const w = closestWindow(data.windows);
  if (!w) return null;
  const eta = burnEta(history[burnKey(provider, w.label)] ?? [], now, w.resetsAt);
  if (eta !== null) {
    return { text: `⚠ ${new Intl.DateTimeFormat(locale(), { hour: "2-digit", minute: "2-digit" }).format(new Date(eta))}`, warn: true };
  }
  const left = w.resetsAt - now;
  if (left <= 0) return null;
  const units = { d: t("ai.unit_d"), h: t("ai.unit_h"), m: t("ai.unit_m") };
  return { text: `↻ ${formatReset(left, units)}`, warn: false };
}

/** A ring (one or two concentric) with the percent of the closest window in the middle. */
function Ring({ d, windows, stroke }: { d: number; windows: AiWindow[]; stroke: number }) {
  const gap = 3;
  const ring = (w: AiWindow | null, inset: number, key: string) => {
    const { r, c } = ringGeometry(d, stroke, inset);
    const pct = w ? Math.min(100, Math.max(0, w.pct)) : 0;
    return (
      <g key={key} transform={`rotate(-90 ${d / 2} ${d / 2})`}>
        <circle cx={d / 2} cy={d / 2} r={r} fill="none" strokeWidth={stroke} stroke="rgba(127,127,127,0.25)" />
        {w && pct > 0 && <circle cx={d / 2} cy={d / 2} r={r} fill="none" strokeWidth={stroke} strokeLinecap="round" stroke={levelColor(w.pct)} strokeDasharray={`${(c * pct) / 100} ${c}`} />}
      </g>
    );
  };
  const outer = windows[0] ?? null;
  const inner = windows[1] ?? null;
  return (
    <svg width={d} height={d} viewBox={`0 0 ${d} ${d}`} role="progressbar" aria-valuemin={0} aria-valuemax={100} aria-valuenow={outer ? Math.round(outer.pct) : 0}>
      {ring(outer, 0, "o")}
      {inner && ring(inner, stroke + gap, "i")}
    </svg>
  );
}

/** The percent of the ring: 15 px if it fits inside the ring, else the 11 px minimum, else nothing (never cut). */
function ringCenter(d: number, stroke: number, rings: number, text: string, big: number): { text: string; px: number } | null {
  const free = d - 2 * (stroke * rings + (rings - 1) * 3) - 4;
  for (const px of [big, 15, MIN_TEXT]) if (px <= big && textWidth(text, px) <= free) return { text, px };
  return null;
}

function RingCell({ entry, width, height, mode, now, history }: { entry: Entry; width: number; height: number; mode: "rings1" | "rings2"; now: number; history: AiHistory }) {
  const { provider, data } = entry;
  const windows = data ? (mode === "rings1" ? data.windows.slice(0, 1) : data.windows) : [];
  const stroke = mode === "rings1" ? 4 : 6;
  const rings = windows.length > 1 ? 2 : 1;
  const closest = closestWindow(windows);
  const text = closest ? `${Math.round(closest.pct)}%` : "—";
  const markSize = mode === "rings1" ? 12 : 14;
  const rowH = markSize + 4;
  const withRow = height - rowH >= 36;
  const d = Math.max(0, Math.min(width - 4, withRow ? height - rowH : height));
  const big = mode === "rings1" ? 15 : 18;
  const centre = ringCenter(d, stroke, rings, text, big);
  const line = mode === "rings2" && data ? resetLine(provider, data, now, history) : null;
  const lineFits = line !== null && markSize + 4 + textWidth(line.text, 12) <= width;
  return (
    <div className="ai-cell" style={{ width }}>
      <div className="ai-ring-box" style={{ width: d, height: d }}>
        <Ring d={d} windows={windows} stroke={stroke} />
        {centre && <span className="ai-ring-pct" style={{ fontSize: centre.px }}>{centre.text}</span>}
      </div>
      {withRow && (
        <div className="ai-cell-row" style={{ height: markSize }}>
          <ProviderMark provider={provider} size={markSize} dim={data === null} />
          {lineFits && line && <span className="ai-reset" style={{ color: line.warn ? LEVEL_COLOR.warning : undefined }}>{line.text}</span>}
        </div>
      )}
    </div>
  );
}

function Bar({ w, units, labels }: { w: AiWindow; units: { d: string; h: string; m: string }; labels: boolean }) {
  return (
    <div className="ai-line">
      {labels && <span className="ai-label">{windowLabel(w.label, units)}</span>}
      <span className="ai-bar" role="progressbar" aria-valuemin={0} aria-valuemax={100} aria-valuenow={Math.round(w.pct)}>
        <span className="ai-fill" style={{ width: `${Math.min(100, Math.max(0, w.pct))}%`, background: levelColor(w.pct) }} />
      </span>
      <span className="ai-pct">{Math.round(w.pct)}%</span>
    </div>
  );
}

function RowBlock({ entry, width, height, now, history, hint }: { entry: Entry; width: number; height: number; now: number; history: AiHistory; hint: boolean }) {
  const { provider, data } = entry;
  const units = { d: t("ai.unit_d"), h: t("ai.unit_h"), m: t("ai.unit_m") };
  const windows = data ? data.windows.slice(0, 2) : [];
  const line = data ? resetLine(provider, data, now, history) : null;
  const fit = rowsFit(width, height, windows.length > 1, line?.text ?? "");
  let hintNode: ReactNode = null;
  if (data === null && provider === "claude" && hint && height >= 36 && textWidth(t("ai.no_data_hint"), 12) <= width - 30) hintNode = <span className="ai-hint">{t("ai.no_data_hint")}</span>;
  return (
    <div className="ai-block" style={{ height }}>
      <ProviderMark provider={provider} size={18} dim={data === null} />
      <div className="ai-block-main">
        {windows.length === 0 && <div className="ai-line"><span className="ai-pct ai-dash">—</span></div>}
        {windows.map((w) => <Bar key={w.label} w={w} units={units} labels={fit.labels} />)}
        {fit.reset && line && <div className="ai-reset" style={{ color: line.warn ? LEVEL_COLOR.warning : undefined }}>{line.text}</div>}
        {hintNode}
      </div>
    </div>
  );
}

/**
 * The AI limits widget (DATA_MODEL.md 6.8, DESIGN.md 4.11): a figure and a shape instead of words. 2x1: a ring per provider;
 * 2x2: concentric rings; from 3x2: two bars per provider; 4x2 adds the Claude Code sessions. Whatever does not fit is left out.
 * A tap opens the sheet with the details.
 */
export default function Ai({ item }: { item: AiItem }) {
  const { ai, onOpenPerm, onOpenPermHistory } = useDeckEnv();
  const { side, gap } = useDeckMetrics();
  const history = useAiHistory();
  const show = item.show ?? "both";
  const now = Date.now();
  const mode = aiMode(item.w, item.h);
  const claude = ai?.claude ?? null;
  const codex = ai?.codex ?? null;
  const pending = ai?.pending.length ?? 0;
  const entries: Entry[] = [];
  if (show === "both" || show === "claude") entries.push({ provider: "claude", data: claude });
  if ((show === "both" || show === "codex") && codex) entries.push({ provider: "codex", data: codex });
  // The tile panel keeps its own padding (0.08 of the side) and a 1 px border (deck.css): the widget has what is left.
  const inset = Math.round(0.08 * side) + 1;
  const W = item.w * side + (item.w - 1) * gap - 2 * inset;
  const H = item.h * side + (item.h - 1) * gap - 2 * inset;
  const withSessions = item.w >= 4 && item.h >= 2 && (ai?.sessions.length ?? 0) > 0;
  const avail = W - 2 * PAD;
  const innerW = withSessions ? Math.round(avail * 0.56) : avail;
  const innerH = H - 2 * PAD;
  const waiting = new Set((ai?.pending ?? []).map((p) => p.sessionId));
  const sessions = withSessions ? (ai?.sessions ?? []).slice(0, 3) : [];
  const sessionsW = avail - innerW - 8; // the gap between the limits and the sessions

  let body: ReactNode;
  if (entries.length === 0) {
    body = null;
  } else if (mode === "rows") {
    const n = entries.length;
    const blockH = Math.floor((innerH - 6 * (n - 1)) / n);
    body = (
      <div className="ai-rows" style={{ gap: 6 }}>
        {entries.map((e) => <RowBlock key={e.provider} entry={e} width={innerW} height={blockH} now={now} history={history} hint={item.w >= 3} />)}
      </div>
    );
  } else {
    const cellW = Math.floor((innerW - 6 * (entries.length - 1)) / entries.length);
    body = (
      <div className="ai-rings" style={{ gap: 6 }}>
        {entries.map((e) => <RingCell key={e.provider} entry={e} width={cellW} height={innerH} mode={mode} now={now} history={history} />)}
      </div>
    );
  }

  return (
    <div className="ai" style={{ padding: PAD }} onClick={() => onOpenPermHistory?.()}>
      <div className="ai-limits" style={{ width: innerW }}>{body}</div>
      {sessions.length > 0 && (
        <div className="ai-sessions" style={{ width: sessionsW }}>
          {sessions.map((s) => {
            const meta = s.ctxPct !== undefined ? t("ai.ctx", { pct: s.ctxPct }) : "";
            const nameW = textWidth(s.project, 13);
            if (nameW > sessionsW - 12) return null; // the name does not fit: the row is left out, not cut
            const metaFits = meta !== "" && nameW + 8 + textWidth(meta, 12) <= sessionsW - 12;
            return (
              <div key={s.id} className="ai-session">
                {waiting.has(s.id) && <span className="ai-wait-dot" style={{ background: LEVEL_COLOR.warning }} />}
                <span className="ai-project">{s.project}</span>
                {metaFits && <span className="ai-meta">{meta}</span>}
              </div>
            );
          })}
        </div>
      )}
      {pending > 0 && (
        <button type="button" className="ai-badge" aria-label={t("ai.waiting", { n: pending })} onClick={(e) => { e.stopPropagation(); onOpenPerm?.(); }}>
          {pending}
        </button>
      )}
    </div>
  );
}
