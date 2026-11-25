import { useEffect, useLayoutEffect, useRef, useState } from "react";
import { LANGUAGE_NAMES, LANGUAGES, t, type Language } from "@decka/shared";
import { orbStateFromIsland, type IslandEvent, type IslandVoicePhase } from "./islandTypes";
import { compactWidth, currentWraps } from "./compactWidth";
import ParticlesOrb from "./ParticlesOrb";
import "./island.css";

export interface IslandProps {
  event: IslandEvent;
  theme?: "light" | "dark";
  colorFrom?: string;
  colorTo?: string;
  /** Preview helper for hover state (normally handled via CSS :hover). */
  hovered?: boolean;
  /** Preview progress 0..1 of the solo -> content transition for deterministic inspection/snapshots. */
  transitionPreview?: number;
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

const FLIP_MS = 320;
const ORB_SOLO_SIZE = 140;
const ORB_CONTENT_SIZE = 72;

export function hasVoiceContent(event: IslandEvent): boolean {
  const v = event.voice;
  if (!v) return false;
  return Boolean(v.heard || v.reply || v.slow);
}

export function expandedStatusText(phase?: IslandVoicePhase): string {
  switch (phase) {
    case "listening":
      return t("voice.listening");
    case "recognizing":
      return t("voice.recognizing");
    case "thinking":
      return t("voice.thinking");
    case "speaking":
      return t("island.speaking");
    case "done":
      return t("voice.done");
    case "error":
      return t("voice.error");
    case "loading":
    case "idle":
    default:
      return t("island.connected");
  }
}

function prefersReducedMotion(): boolean {
  return typeof window !== "undefined" && typeof window.matchMedia === "function"
    ? window.matchMedia("(prefers-reduced-motion: reduce)").matches
    : false;
}

/**
 * Floating Island surface for PC (TASKS.md §T26.5b):
 * - `hidden`, `tab` (96x6 bar in 160x24 hit zone)
 * - `compact` (360x56 / 72, 32px orb)
 * - `expanded` solo (260x230, 140px orb + centered status line) and with content (520 x <=360, 72px orb left + right content column),
 *   connected by a 320ms FLIP transition keeping the orb canvas alive until completion.
 */
export default function Island({
  event,
  theme = "dark",
  colorFrom = "#22d3ee",
  colorTo = "#d946ef",
  hovered = false,
  transitionPreview,
  onAction,
  onChipClick,
  onStopSlow,
  onTabHover,
}: IslandProps) {
  const { mode, lyrics, voice } = event;
  const [langMenuOpen, setLangMenuOpen] = useState(false);
  const langRef = useRef<HTMLDivElement>(null);

  const rawHasContent = mode === "expanded" && hasVoiceContent(event);
  const [stickyContent, setStickyContent] = useState(rawHasContent);
  const prevModeRef = useRef(mode);
  const prevPhaseRef = useRef<IslandVoicePhase | undefined>(voice?.phase);

  // Stay in content layout until leaving expanded OR starting a new voice request (listening/loading with no text).
  if (mode !== "expanded") {
    if (stickyContent) setStickyContent(false);
  } else if (rawHasContent) {
    if (!stickyContent) setStickyContent(true);
  } else if (
    stickyContent &&
    (voice?.phase === "listening" || voice?.phase === "loading") &&
    prevPhaseRef.current !== "listening" &&
    prevPhaseRef.current !== "loading"
  ) {
    setStickyContent(false);
  }

  const previewRatio = typeof transitionPreview === "number" ? transitionPreview : -1;
  const gtZero = previewRatio > 0;
  const ltOne = previewRatio < 1;
  const isPreviewMid = mode === "expanded" && gtZero && ltOne;
  const isContent = mode === "expanded" && (rawHasContent || stickyContent || isPreviewMid);

  const panelRef = useRef<HTMLDivElement>(null);
  const orbSlotRef = useRef<HTMLDivElement>(null);
  const orbWrapRef = useRef<HTMLDivElement>(null);
  const prevIsContentRef = useRef(isContent);
  const prevSoloOrbRectRef = useRef<DOMRect | null>(null);
  const flipTimerRef = useRef<number | null>(null);
  const [flipping, setFlipping] = useState(false);
  const [animateContent, setAnimateContent] = useState(false);

  useEffect(() => {
    prevModeRef.current = mode;
    prevPhaseRef.current = voice?.phase;
  }, [mode, voice?.phase]);

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

  // Measure solo orb geometry and run 320 ms FLIP transition when switching solo -> content.
  useLayoutEffect(() => {
    if (mode !== "expanded" || isPreviewMid) {
      prevIsContentRef.current = isContent;
      if (flipping) setFlipping(false);
      if (animateContent) setAnimateContent(false);
      return;
    }

    const panelEl = panelRef.current;
    const slotEl = orbSlotRef.current;
    const wrapEl = orbWrapRef.current;

    if (!isContent) {
      if (wrapEl) prevSoloOrbRectRef.current = wrapEl.getBoundingClientRect();
      prevIsContentRef.current = false;
      if (flipping) setFlipping(false);
      if (animateContent) setAnimateContent(false);
      return;
    }

    const justEnteredContent = !prevIsContentRef.current;
    prevIsContentRef.current = true;
    if (!justEnteredContent || !panelEl || !slotEl || !wrapEl) return;

    if (prefersReducedMotion()) {
      panelEl.style.width = "";
      panelEl.style.height = "";
      panelEl.style.transition = "";
      wrapEl.style.transform = "";
      wrapEl.style.transition = "";
      setFlipping(false);
      setAnimateContent(false);
      return;
    }

    if (flipTimerRef.current !== null) {
      window.clearTimeout(flipTimerRef.current);
      flipTimerRef.current = null;
    }

    const targetHeight = Math.min(360, Math.max(112, panelEl.scrollHeight || 180));
    const slotRect = slotEl.getBoundingClientRect();
    const firstRect = prevSoloOrbRectRef.current;

    // Fallback delta when unmeasured in non-layout test environments matches geometric solo center -> 520px left slot.
    let dx = 74;
    let dy = 40;
    if (firstRect && firstRect.width > 0 && slotRect.width > 0) {
      dx = firstRect.left + firstRect.width / 2 - (slotRect.left + slotRect.width / 2);
      dy = firstRect.top + firstRect.height / 2 - (slotRect.top + slotRect.height / 2);
    }

    // Frame 0: hold 140px canvas backing & invert orb + panel box to exact solo geometry.
    setFlipping(true);
    setAnimateContent(true);

    panelEl.style.transition = "none";
    panelEl.style.width = "260px";
    panelEl.style.height = "230px";

    wrapEl.style.transition = "none";
    wrapEl.style.transform = `translate(${dx}px, ${dy}px) scale(1)`;

    void panelEl.offsetWidth;

    const easing = `cubic-bezier(0.2, 0.8, 0.2, 1)`;
    panelEl.style.transition = `width ${FLIP_MS}ms ${easing}, height ${FLIP_MS}ms ${easing}, opacity 150ms ease`;
    panelEl.style.width = "520px";
    panelEl.style.height = `${targetHeight}px`;

    wrapEl.style.transition = `transform ${FLIP_MS}ms ${easing}`;
    wrapEl.style.transform = `translate(0px, 0px) scale(${ORB_CONTENT_SIZE / ORB_SOLO_SIZE})`;

    flipTimerRef.current = window.setTimeout(() => {
      flipTimerRef.current = null;
      if (panelRef.current) {
        panelRef.current.style.transition = "";
        panelRef.current.style.width = "";
        panelRef.current.style.height = "";
      }
      if (orbWrapRef.current) {
        orbWrapRef.current.style.transition = "";
        orbWrapRef.current.style.transform = "";
      }
      setFlipping(false);
    }, FLIP_MS);
  }, [mode, isContent, isPreviewMid]);

  useEffect(
    () => () => {
      if (flipTimerRef.current !== null) window.clearTimeout(flipTimerRef.current);
    },
    [],
  );

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
    const lyricsShown = hasLyrics && !voiceActive;
    const wraps = lyricsShown && currentWraps(lyrics?.current);
    const panelWidth = lyricsShown ? compactWidth(lyrics?.current, lyrics?.next) : undefined;
    const slow = voice?.slow;
    const slowOverdue = slow && (slow.elapsedSec ?? 0) > slow.etaSec * 2;
    const slowPct = slow ? Math.min(95, Math.round(((slow.elapsedSec ?? 0) / Math.max(1, slow.etaSec)) * 100)) : 0;

    return (
      <div className="island-host" data-island-theme={theme}>
        <div
          className={`island-panel island-compact${hasTwoLines ? " two-lines" : ""}${wraps ? " wraps" : ""}${hasLyrics && !voiceActive ? " is-lyrics" : ""}${hovered ? " is-hovered" : ""}`}
          style={panelWidth ? { width: panelWidth } : undefined}
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

  // mode === "expanded": Solo (260x230, 140px orb) vs Content (520px, 72px orb left + right column).
  const slow = voice?.slow;
  const slowOverdue = slow && (slow.elapsedSec ?? 0) > slow.etaSec * 2;
  const slowPct = slow ? Math.min(95, Math.round(((slow.elapsedSec ?? 0) / Math.max(1, slow.etaSec)) * 100)) : 0;
  const hasActions = Boolean(voice?.actions && voice.actions.length > 0);
  const showLang = Boolean(voice?.actions?.some((a) => a === "lang" || a.startsWith("lang:")));
  const statusLabel = expandedStatusText(voice?.phase);

  // Keep 140px canvas live throughout solo, active FLIP transition, or preview snapshot; shrink to 72 at rest.
  const orbCanvasSize = !isContent || flipping || isPreviewMid ? ORB_SOLO_SIZE : ORB_CONTENT_SIZE;

  // Sample cubic-bezier(0.2, 0.8, 0.2, 1) and 120ms-delayed 160ms content fade for deterministic preview snapshots.
  const easeOutCurve = (p: number) => 1 - Math.pow(1 - Math.max(0, Math.min(1, p)), 3);
  const previewT = isPreviewMid ? previewRatio : 0;
  const previewEase = easeOutCurve(previewT);
  const contentT = Math.max(0, Math.min(1, (previewT * FLIP_MS - 120) / 160));
  const previewPanelStyle = isPreviewMid
    ? {
        width: `${Math.round(260 + (520 - 260) * previewEase)}px`,
        height: `${Math.round(230 + (186 - 230) * previewEase)}px`,
      }
    : undefined;
  const previewOrbWrapStyle = isPreviewMid
    ? {
        transform: `translate(${(74 * (1 - previewEase)).toFixed(1)}px, ${(40 * (1 - previewEase)).toFixed(1)}px) scale(${(1 + (ORB_CONTENT_SIZE / ORB_SOLO_SIZE - 1) * previewEase).toFixed(4)})`,
      }
    : undefined;
  const previewContentStyle = isPreviewMid
    ? {
        opacity: contentT,
        transform: `translateX(${(8 * (1 - contentT)).toFixed(1)}px)`,
      }
    : undefined;

  return (
    <div className="island-host" data-island-theme={theme}>
      <div
        ref={panelRef}
        className={`island-panel island-expanded ${isContent ? "is-content" : "is-solo"}${flipping || isPreviewMid ? " is-transitioning" : ""}${hovered ? " is-hovered" : ""}`}
        style={previewPanelStyle}
        role="dialog"
        aria-label={t("orb.aria")}
      >
        <div
          ref={orbSlotRef}
          className={`island-orb-slot ${isContent ? "is-content" : "is-solo"}`}
        >
          <div
            ref={orbWrapRef}
            className={`island-orb-wrap expanded${isError ? " is-error" : ""}`}
            style={previewOrbWrapStyle}
          >
            <ParticlesOrb
              state={orbState}
              size={orbCanvasSize}
              colorFrom={colorFrom}
              colorTo={colorTo}
              level={voice?.level}
            />
          </div>
        </div>

        {!isContent ? (
          <div className="island-solo-status">{statusLabel}</div>
        ) : (
          <div
            className={`island-content-col${animateContent ? " is-entering" : ""}`}
            style={previewContentStyle}
          >
            {slow ? (
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
            ) : (
              voiceActive &&
              !voice?.reply &&
              voice?.phase !== "done" &&
              voice?.phase !== "idle" && <div className="island-content-status">{statusLabel}</div>
            )}

            {voice?.heard && <div className="island-heard">“{voice.heard}”</div>}

            {voice?.reply && <div className="island-reply">{voice.reply}</div>}

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
        )}
      </div>
    </div>
  );
}
