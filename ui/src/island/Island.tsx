import { useEffect, useLayoutEffect, useRef, useState } from "react";
import {
  Check,
  ChevronDown,
  ClipboardPaste,
  Code,
  Copy,
  HelpCircle,
  Languages,
  Mic,
  Music,
  Pause,
  Play,
  RefreshCw,
  Reply,
  SkipBack,
  SkipForward,
  Square,
  Volume2,
  VolumeX,
} from "lucide-react";
import { LANGUAGE_NAMES, LANGUAGES, locale, t, type Language } from "@decka/shared";
import { formatTime } from "../widgets/mediaPos";
import {
  orbStateFromIsland,
  type IslandContextChip,
  type IslandMediaAction,
  type IslandMusicEvent,
  type IslandVoicePhase,
} from "./islandTypes";
import ParticlesOrb from "./ParticlesOrb";
import "./island.css";

export interface IslandProps {
  event: IslandMusicEvent;
  theme?: "light" | "dark";
  colorFrom?: string;
  colorTo?: string;
  /** Preview helper for hover state (normally handled via CSS :hover). */
  hovered?: boolean;
  /** Preview progress 0..1 of the solo -> content transition for deterministic inspection/snapshots. */
  transitionPreview?: number;
  /** Preview helpers for deterministic answer-toolbar screenshots. */
  hoveredAction?: string;
  copiedPreview?: boolean;
  langMenuOpenPreview?: boolean;
  onAction?: (kind: string) => void;
  onChipClick?: (command: string) => void;
  onMediaControl?: (action: IslandMediaAction) => void;
  onStopSlow?: () => void;
  onTabHover?: () => void;
}

const FLIP_MS = 320;
const COPY_FEEDBACK_MS = 1200;
const ORB_SOLO_SIZE = 140;
const ORB_CONTENT_SIZE = 72;

export function hasVoiceContent(event: IslandMusicEvent): boolean {
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

export function chipIconKind(chip: IslandContextChip): "code" | "translate" | "reply" | "help" {
  const hay = `${chip.command} ${chip.label}`.toLowerCase();
  if (hay.includes("translate") || hay.includes("перевед")) return "translate";
  if (hay.includes("reply") || hay.includes("ответ")) return "reply";
  if (hay.includes("copy") || hay.includes("code") || hay.includes("код")) return "code";
  return "help";
}

function renderChipIcon(kind: "code" | "translate" | "reply" | "help") {
  switch (kind) {
    case "code":
      return <Code size={14} strokeWidth={1.75} aria-hidden="true" />;
    case "translate":
      return <Languages size={14} strokeWidth={1.75} aria-hidden="true" />;
    case "reply":
      return <Reply size={14} strokeWidth={1.75} aria-hidden="true" />;
    case "help":
      return <HelpCircle size={14} strokeWidth={1.75} aria-hidden="true" />;
  }
}

function prefersReducedMotion(): boolean {
  return typeof window !== "undefined" && typeof window.matchMedia === "function"
    ? window.matchMedia("(prefers-reduced-motion: reduce)").matches
    : false;
}

/**
 * Floating Island surface for PC (TASKS.md §T26.5b / §T26.F.9):
 * - `hidden`, `tab` (96x6 bar in 160x24 hit zone)
 * - `compact` (360x56 / 72, 32px orb)
 * - `expanded` solo (260x230, 140px orb + centered status line) and with content (520 x <=360, 72px orb left + right content column),
 *   with icon-first answer bar (lucide-react 18px, 1.75 stroke) and context chips.
 */
export default function Island({
  event,
  theme = "dark",
  colorFrom = "#22d3ee",
  colorTo = "#d946ef",
  hovered = false,
  transitionPreview,
  hoveredAction,
  copiedPreview = false,
  langMenuOpenPreview = false,
  onAction,
  onChipClick,
  onMediaControl,
  onStopSlow,
  onTabHover,
}: IslandProps) {
  const { mode, lyrics, voice } = event;
  const [langMenuOpen, setLangMenuOpen] = useState(false);
  const [copied, setCopied] = useState(false);
  const copyTimerRef = useRef<number | null>(null);
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
    setCopied(false);
    if (copyTimerRef.current !== null) {
      window.clearTimeout(copyTimerRef.current);
      copyTimerRef.current = null;
    }
  }, [voice?.reply]);

  useEffect(
    () => () => {
      if (copyTimerRef.current !== null) window.clearTimeout(copyTimerRef.current);
    },
    [],
  );

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

    let dx = 74;
    let dy = 40;
    if (firstRect && firstRect.width > 0 && slotRect.width > 0) {
      dx = firstRect.left + firstRect.width / 2 - (slotRect.left + slotRect.width / 2);
      dy = firstRect.top + firstRect.height / 2 - (slotRect.top + slotRect.height / 2);
    }

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
    const hasActiveLyric = Boolean(lyrics?.current);
    const hasSong = !voiceActive && Boolean(lyrics);
    const countdownKnown = !hasActiveLyric && typeof lyrics?.nextInSec === "number" && lyrics.nextInSec >= 0;
    const countdownUnknown = !hasActiveLyric && Boolean(lyrics?.lyricsPending || lyrics?.nextInSec === null);
    const trackSummary =
      lyrics?.title && lyrics?.artist
        ? `${lyrics.title} — ${lyrics.artist}`
        : (lyrics?.title ?? lyrics?.artist ?? "");
    const hasTwoLines = hasSong
      ? hasActiveLyric
        ? Boolean(lyrics?.next)
        : countdownKnown || countdownUnknown || Boolean(lyrics?.artist)
      : false;
    const slow = voice?.slow;
    const slowOverdue = slow && (slow.elapsedSec ?? 0) > slow.etaSec * 2;
    const slowPct = slow ? Math.min(95, Math.round(((slow.elapsedSec ?? 0) / Math.max(1, slow.etaSec)) * 100)) : 0;
    const playPauseLabel = lyrics?.playing ? t("island.media_pause") : t("island.media_play");
    const muteLabel = lyrics?.muted ? t("island.media_unmute") : t("island.media_mute");

    return (
      <div className="island-host" data-island-theme={theme}>
        <div
          className={`island-panel island-compact${hasTwoLines ? " two-lines" : ""}${hasSong ? " is-lyrics is-song" : ""}${hovered ? " is-hovered" : ""}`}
          role="status"
        >
          <div className={`island-orb-wrap compact${isError ? " is-error" : ""}`}>
            <ParticlesOrb
              state={orbState}
              size={32}
              colorFrom={colorFrom}
              colorTo={colorTo}
              level={hasSong ? lyrics?.musicLevel : voice?.level}
              particleCount={hasSong ? 140 : undefined}
              maxFps={hasSong ? 20 : undefined}
              amplitude={hasSong ? 0.6 : 1}
              staticFrame={!voiceActive && !lyrics?.playing}
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
            ) : hasSong ? (
              hasActiveLyric ? (
                <>
                  <div className="island-lyric-current">{lyrics?.current}</div>
                  {lyrics?.next && <div className="island-lyric-next">{lyrics.next}</div>}
                </>
              ) : countdownKnown ? (
                <>
                  <div className="island-lyric-current island-song-title-row">
                    <Music size={12} strokeWidth={1.75} className="island-song-icon" aria-hidden="true" />
                    <span className="island-song-title-text">{trackSummary}</span>
                  </div>
                  <div className="island-lyric-next">
                    {t("island.lyrics_in", { time: formatTime(Math.max(0, Math.round(lyrics?.nextInSec ?? 0))) })}
                  </div>
                </>
              ) : countdownUnknown ? (
                <>
                  <div className="island-lyric-current island-song-title-row">
                    <Music size={12} strokeWidth={1.75} className="island-song-icon" aria-hidden="true" />
                    <span className="island-song-title-text">{trackSummary}</span>
                  </div>
                  <div className="island-lyric-next island-lyrics-dots" aria-label={t("island.lyrics_waiting")}>
                    <span className="island-lyrics-dot" />
                    <span className="island-lyrics-dot" />
                    <span className="island-lyrics-dot" />
                  </div>
                </>
              ) : (
                <>
                  <div className="island-lyric-current island-song-title-row">
                    <Music size={12} strokeWidth={1.75} className="island-song-icon" aria-hidden="true" />
                    <span className="island-song-title-text">{lyrics?.title ?? ""}</span>
                  </div>
                  {lyrics?.artist && <div className="island-lyric-next">{lyrics.artist}</div>}
                </>
              )
            ) : (
              <div className="island-lyric-current">{voice?.reply ?? voice?.heard ?? t("voice.listening")}</div>
            )}
          </div>
          {hasSong && (
            <div className="island-compact-controls">
              <button
                type="button"
                className="island-media-btn"
                title={t("island.media_prev")}
                aria-label={t("island.media_prev")}
                onClick={() => onMediaControl?.("prev")}
              >
                <SkipBack size={16} strokeWidth={1.75} aria-hidden="true" />
              </button>
              <button
                type="button"
                className="island-media-btn"
                title={playPauseLabel}
                aria-label={playPauseLabel}
                onClick={() => onMediaControl?.("play_pause")}
              >
                {lyrics?.playing ? (
                  <Pause size={16} strokeWidth={1.75} aria-hidden="true" />
                ) : (
                  <Play size={16} strokeWidth={1.75} aria-hidden="true" />
                )}
              </button>
              <button
                type="button"
                className="island-media-btn"
                title={t("island.media_next")}
                aria-label={t("island.media_next")}
                onClick={() => onMediaControl?.("next")}
              >
                <SkipForward size={16} strokeWidth={1.75} aria-hidden="true" />
              </button>
              <button
                type="button"
                className="island-media-btn"
                title={muteLabel}
                aria-label={muteLabel}
                onClick={() => onMediaControl?.("mute")}
              >
                {lyrics?.muted ? (
                  <VolumeX size={16} strokeWidth={1.75} aria-hidden="true" />
                ) : (
                  <Volume2 size={16} strokeWidth={1.75} aria-hidden="true" />
                )}
              </button>
            </div>
          )}
        </div>
      </div>
    );
  }

  // mode === "expanded": Solo (260x230, 140px orb) vs Content (520px, 72px orb left + right column).
  const slow = voice?.slow;
  const slowOverdue = slow && (slow.elapsedSec ?? 0) > slow.etaSec * 2;
  const slowPct = slow ? Math.min(95, Math.round(((slow.elapsedSec ?? 0) / Math.max(1, slow.etaSec)) * 100)) : 0;
  const actions = voice?.actions ?? [];
  const hasCopy = actions.includes("copy");
  const hasInsert = actions.includes("insert");
  const hasSpeak = actions.includes("speak");
  const hasAgain = actions.includes("again");
  const showLang = actions.some((a) => a === "lang" || a.startsWith("lang:"));
  const hasGroup1 = hasCopy || hasInsert || hasSpeak;
  const hasActions = hasGroup1 || hasAgain || showLang;
  const statusLabel = expandedStatusText(voice?.phase);
  const currentLang = locale();
  const isCopied = copied || copiedPreview;
  const isSpeaking = voice?.phase === "speaking";
  const isAgainSpinning = Boolean(voiceActive && (voice?.phase === "thinking" || voice?.phase === "recognizing" || voice?.phase === "loading"));
  const menuOpen = langMenuOpen || langMenuOpenPreview;

  const orbCanvasSize = !isContent || flipping || isPreviewMid ? ORB_SOLO_SIZE : ORB_CONTENT_SIZE;

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

  const handleCopy = () => {
    setCopied(true);
    if (copyTimerRef.current !== null) window.clearTimeout(copyTimerRef.current);
    copyTimerRef.current = window.setTimeout(() => {
      copyTimerRef.current = null;
      setCopied(false);
    }, COPY_FEEDBACK_MS);
    onAction?.("copy");
  };

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

            {voice?.heard && (
              <div className="island-heard">
                <Mic size={12} strokeWidth={1.75} className="island-heard-mic" aria-hidden="true" />
                <span className="island-heard-text">{voice.heard}</span>
              </div>
            )}

            {voice?.reply && <div className="island-reply">{voice.reply}</div>}

            {voice?.chips && voice.chips.length > 0 && (
              <div className="island-chips">
                {voice.chips.map((chip, idx) => (
                  <button
                    key={idx}
                    type="button"
                    className="island-chip"
                    onClick={() => onChipClick?.(chip.command)}
                  >
                    {renderChipIcon(chipIconKind(chip))}
                    <span>{chip.label}</span>
                  </button>
                ))}
              </div>
            )}

            {hasActions && (
              <div className="island-actions">
                {hasCopy && (
                  <button
                    type="button"
                    className={`island-action-btn${isCopied ? " is-copied" : ""}${hoveredAction === "copy" ? " is-hovered" : ""}`}
                    title={isCopied ? t("island.copied") : t("island.copy")}
                    aria-label={isCopied ? t("island.copied") : t("island.copy")}
                    onClick={handleCopy}
                  >
                    {isCopied ? <Check size={18} strokeWidth={1.75} aria-hidden="true" /> : <Copy size={18} strokeWidth={1.75} aria-hidden="true" />}
                  </button>
                )}
                {hasInsert && (
                  <button
                    type="button"
                    className={`island-action-btn${hoveredAction === "insert" ? " is-hovered" : ""}`}
                    title={t("island.insert")}
                    aria-label={t("island.insert")}
                    onClick={() => onAction?.("insert")}
                  >
                    <ClipboardPaste size={18} strokeWidth={1.75} aria-hidden="true" />
                  </button>
                )}
                {hasSpeak && (
                  <button
                    type="button"
                    className={`island-action-btn${hoveredAction === "speak" ? " is-hovered" : ""}`}
                    title={isSpeaking ? t("island.stop_speaking") : t("island.speak")}
                    aria-label={isSpeaking ? t("island.stop_speaking") : t("island.speak")}
                    onClick={() => onAction?.("speak")}
                  >
                    {isSpeaking ? <Square size={18} strokeWidth={1.75} aria-hidden="true" /> : <Volume2 size={18} strokeWidth={1.75} aria-hidden="true" />}
                  </button>
                )}

                {hasGroup1 && hasAgain && <span className="island-actions-sep" aria-hidden="true" />}

                {hasAgain && (
                  <button
                    type="button"
                    className={`island-action-btn${hoveredAction === "again" ? " is-hovered" : ""}`}
                    title={t("island.again")}
                    aria-label={t("island.again")}
                    onClick={() => onAction?.("again")}
                  >
                    <RefreshCw
                      size={18}
                      strokeWidth={1.75}
                      className={isAgainSpinning ? "island-icon-spin" : undefined}
                      aria-hidden="true"
                    />
                  </button>
                )}

                {showLang && (
                  <div className="island-lang-wrap" ref={langRef}>
                    <button
                      type="button"
                      className={`island-action-btn island-lang-btn${hoveredAction === "lang" ? " is-hovered" : ""}`}
                      title={t("island.language")}
                      aria-label={t("island.language")}
                      aria-expanded={menuOpen}
                      onClick={() => setLangMenuOpen((v) => !v)}
                    >
                      <Languages size={18} strokeWidth={1.75} aria-hidden="true" />
                      <span className="island-lang-code">{currentLang.toUpperCase()}</span>
                      <ChevronDown size={14} strokeWidth={1.75} aria-hidden="true" />
                    </button>
                    {menuOpen && (
                      <div className="island-lang-menu" role="menu">
                        {LANGUAGES.map((code: Language) => (
                          <button
                            key={code}
                            type="button"
                            role="menuitem"
                            className={`island-lang-item${code === currentLang ? " is-current" : ""}`}
                            onClick={() => {
                              setLangMenuOpen(false);
                              onAction?.(`lang:${code}`);
                            }}
                          >
                            <span className="island-lang-item-check">
                              {code === currentLang ? <Check size={14} strokeWidth={1.75} aria-hidden="true" /> : null}
                            </span>
                            <span>{LANGUAGE_NAMES[code]}</span>
                          </button>
                        ))}
                      </div>
                    )}
                  </div>
                )}
              </div>
            )}
          </div>
        )}
      </div>
    </div>
  );
}
