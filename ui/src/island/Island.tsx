import { useEffect, useRef, useState } from "react";
import { LANGUAGE_NAMES, LANGUAGES, t, type Language } from "@decka/shared";
import { orbStateFromIsland, type IslandEvent } from "./islandTypes";
import ParticlesOrb from "./ParticlesOrb";
import "./island.css";

export interface IslandProps {
  event: IslandEvent;
  theme?: "light" | "dark";
  colorFrom?: string;
  colorTo?: string;
  /** Preview helper for hover state (normally handled via CSS :hover). */
  hovered?: boolean;
  onAction?: (kind: string) => void;
  onChipClick?: (command: string) => void;
  onStopSlow?: () => void;
  onTabHover?: () => void;
}

const SIMPLE_ACTIONS: { kind: string; key: "island.copy" | "island.insert" | "island.speak" | "island.again" }[] = [
  { kind: "copy", key: "island.copy" },
  { kind: "insert", key: "island.insert" },
  { kind: "speak", key: "island.speak" },
  { kind: "again", key: "island.again" },
];

/**
 * Floating Island surface for PC (TASKS.md §T26.5b): handles `hidden`, `tab`, `compact` (lyrics / compact voice),
 * and `expanded` (120px ParticlesOrb, slow-path progress bar, transcript, reply, action buttons, language picker, chips).
 */
export default function Island({
  event,
  theme = "dark",
  colorFrom = "#22d3ee",
  colorTo = "#d946ef",
  hovered = false,
  onAction,
  onChipClick,
  onStopSlow,
  onTabHover,
}: IslandProps) {
  const { mode, lyrics, voice } = event;
  const [langMenuOpen, setLangMenuOpen] = useState(false);
  const langRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!langMenuOpen) return;
    const onDown = (e: PointerEvent) => {
      if (langRef.current && !langRef.current.contains(e.target as Node)) setLangMenuOpen(false);
    };
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") setLangMenuOpen(false);
    };
    window.addEventListener("pointerdown", onDown);
    window.addEventListener("keydown", onKey);
    return () => {
      window.removeEventListener("pointerdown", onDown);
      window.removeEventListener("keydown", onKey);
    };
  }, [langMenuOpen]);

  if (mode === "hidden") return null;

  const { orbState, isError } = orbStateFromIsland(event);
  const voiceActive = Boolean(voice?.active);

  if (mode === "tab") {
    return (
      <div className="island-host" data-island-theme={theme}>
        <div
          className="island-tab-zone"
          onPointerEnter={onTabHover}
          role="region"
          aria-label={t("orb.aria")}
        >
          <div className="island-tab-bar" />
        </div>
      </div>
    );
  }

  if (mode === "compact") {
    const hasLyrics = Boolean(lyrics?.playing && lyrics.current);
    const hasTwoLines = hasLyrics && Boolean(lyrics?.next);
    const slow = voice?.slow;
    const slowOverdue = slow && (slow.elapsedSec ?? 0) > slow.etaSec * 2;
    const slowPct = slow ? Math.min(95, Math.round(((slow.elapsedSec ?? 0) / Math.max(1, slow.etaSec)) * 100)) : 0;

    return (
      <div className="island-host" data-island-theme={theme}>
        <div
          className={`island-panel island-compact${hasTwoLines ? " two-lines" : ""}${hasLyrics && !voiceActive ? " is-lyrics" : ""}${hovered ? " is-hovered" : ""}`}
          role="status"
        >
          <div className={`island-orb-wrap compact${isError ? " is-error" : ""}`}>
            <ParticlesOrb
              state={orbState}
              size={32}
              colorFrom={colorFrom}
              colorTo={colorTo}
              level={voice?.level}
              staticFrame={!voiceActive}
            />
          </div>
          <div className="island-compact-text">
            {voiceActive && slow ? (
              <div className="island-slow">
                <div className="island-slow-row">
                  <span>{slowOverdue ? t("island.slow_almost") : t("island.slow_eta", { s: Math.max(1, Math.round(slow.etaSec)) })}</span>
                  {onStopSlow && (
                    <button type="button" className="island-stop-btn" onClick={onStopSlow}>
                      {t("island.stop")}
                    </button>
                  )}
                </div>
                <div className="island-slow-track">
                  <div className="island-slow-fill" style={{ width: `${Math.max(12, slowPct)}%` }} />
                </div>
              </div>
            ) : hasLyrics ? (
              <>
                <div className="island-lyric-current">{lyrics?.current}</div>
                {lyrics?.next && <div className="island-lyric-next">{lyrics.next}</div>}
              </>
            ) : (
              <div className="island-lyric-current">{voice?.reply ?? voice?.heard ?? t("voice.listening")}</div>
            )}
          </div>
        </div>
      </div>
    );
  }

  // mode === "expanded"
  const slow = voice?.slow;
  const slowOverdue = slow && (slow.elapsedSec ?? 0) > slow.etaSec * 2;
  const slowPct = slow ? Math.min(95, Math.round(((slow.elapsedSec ?? 0) / Math.max(1, slow.etaSec)) * 100)) : 0;
  const hasActions = Boolean(voice?.actions && voice.actions.length > 0);
  const showLang = Boolean(voice?.actions?.some((a) => a === "lang" || a.startsWith("lang:")));

  return (
    <div className="island-host" data-island-theme={theme}>
      <div className={`island-panel island-expanded${hovered ? " is-hovered" : ""}`} role="dialog" aria-label={t("orb.aria")}>
        <div className={`island-orb-wrap expanded${isError ? " is-error" : ""}`}>
          <ParticlesOrb
            state={orbState}
            size={120}
            colorFrom={colorFrom}
            colorTo={colorTo}
            level={voice?.level}
          />
        </div>

        {slow && (
          <div className="island-slow island-slow-expanded">
            <div className="island-slow-row">
              <span>{slowOverdue ? t("island.slow_almost") : t("island.slow_eta", { s: Math.max(1, Math.round(slow.etaSec)) })}</span>
              {onStopSlow && (
                <button type="button" className="island-stop-btn" onClick={onStopSlow}>
                  {t("island.stop")}
                </button>
              )}
            </div>
            <div className="island-slow-track">
              <div className="island-slow-fill" style={{ width: `${Math.max(10, slowPct)}%` }} />
            </div>
          </div>
        )}

        {voice?.heard && (
          <div className="island-heard">“{voice.heard}”</div>
        )}

        {voice?.reply && (
          <div className="island-reply">{voice.reply}</div>
        )}

        {hasActions && (
          <div className="island-actions">
            {SIMPLE_ACTIONS.filter((a) => voice?.actions?.includes(a.kind)).map((a) => (
              <button
                key={a.kind}
                type="button"
                className="island-action-btn"
                onClick={() => onAction?.(a.kind)}
              >
                {t(a.key)}
              </button>
            ))}
            {showLang && (
              <div className="island-lang-wrap" ref={langRef}>
                <button
                  type="button"
                  className="island-action-btn"
                  aria-expanded={langMenuOpen}
                  onClick={() => setLangMenuOpen((v) => !v)}
                >
                  {t("island.language")} ▾
                </button>
                {langMenuOpen && (
                  <div className="island-lang-menu" role="menu">
                    {LANGUAGES.map((code: Language) => (
                      <button
                        key={code}
                        type="button"
                        role="menuitem"
                        className="island-lang-item"
                        onClick={() => {
                          setLangMenuOpen(false);
                          onAction?.(`lang:${code}`);
                        }}
                      >
                        {LANGUAGE_NAMES[code]}
                      </button>
                    ))}
                  </div>
                )}
              </div>
            )}
          </div>
        )}

        {voice?.chips && voice.chips.length > 0 && (
          <div className="island-chips">
            {voice.chips.map((chip, idx) => (
              <button
                key={idx}
                type="button"
                className="island-chip"
                onClick={() => onChipClick?.(chip.command)}
              >
                {chip.label}
              </button>
            ))}
          </div>
        )}
      </div>
    </div>
  );
}
