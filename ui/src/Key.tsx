import { useCallback, useEffect, useRef, useState, type CSSProperties } from "react";
import { useDeckEnv } from "./deckEnv";
import { faceLayout } from "./faceLayout";
import Icon from "./Icon";
import { useTile } from "./useTile";
import { useDeckMetrics } from "./metricsContext";
import { colorVars, parseHex } from "./theme/colors";
import { materialKeyVars } from "./theme/material";

const TAP_SLOP_PX = 20;
const CONFIRM_HOLD_MS = 800;
const OK_MS = 300;
const ERROR_MS = 1500;

type Phase = "idle" | "pressed" | "confirming" | "pending" | "ok" | "error";

export interface KeyProps {
  /** Sent to the PC on a tap (T10.5 states, error toasts); disabled while offline. */
  actionId?: string;
  /** Called on a tap when there is no `actionId` (page navigation, opening a popup group); gets the key's box. */
  onTap?: (box: DOMRect) => void;
  /** Called after the action of this key succeeded (`actionId` + popup: the group opens now); gets the key's box. */
  onPressed?: (box: DOMRect) => void;
  /** Hold 800 ms instead of tapping. */
  confirm?: boolean;
  icon?: string;
  label?: string;
  color?: string;
  /** Key side in px. Without it the key is one grid cell. */
  side?: number;
  /** Small `›` in the corner: the key opens another page. */
  chevron?: boolean;
  /** Small `•••` in the corner: the key opens a popup group. */
  dots?: boolean;
  /** `full`: the icon fills the key like an app icon on a home screen (DESIGN.md 2.1). */
  face?: "standard" | "full";
  /** The variable the key follows does not exist (no OBS, no microphone): the key is drawn at 0.45, and still works (DESIGN.md 2.4). */
  noData?: boolean;
  /** A red dot in the top left corner that blinks once a second: recording or on air (DESIGN.md 2.4). */
  liveDot?: boolean;
}

/** The corner mark of a `full` key sits on a dark disc of 0.2 x side (DESIGN.md 2.1). */
function discStyle(side: number, font: number): CSSProperties {
  const d = 0.2 * side;
  return { top: 0.04 * side, right: 0.04 * side, width: d, height: d, fontSize: font * side, borderRadius: "50%", background: "rgba(0,0,0,0.45)", display: "flex", alignItems: "center", justifyContent: "center", color: "rgba(255,255,255,0.8)" };
}

/** One key with the shared press states: pressed, confirming, pending, ok, error. Used by buttons and widgets. */
export default function Key({ actionId, onTap, onPressed, confirm, icon, label, color, side: fixedSide, chevron, dots, face, noData, liveDot }: KeyProps) {
  const element = useRef<HTMLDivElement>(null);
  const { online, onPress, onPressError, onFeedback, icons } = useDeckEnv();
  const disabled = actionId !== undefined && !online;

  const [phase, setPhaseState] = useState<Phase>("idle");
  const phaseRef = useRef<Phase>("idle");
  const setPhase = useCallback((p: Phase) => {
    phaseRef.current = p;
    setPhaseState(p);
  }, []);

  const start = useRef<{ x: number; y: number } | null>(null);
  const holdTimer = useRef<number | null>(null);
  const resetTimer = useRef<number | null>(null);
  const mounted = useRef(true);
  const metrics = useDeckMetrics();
  const side = fixedSide ?? metrics.side;
  const coloured = color !== undefined && parseHex(color) !== null;
  // `full` face: where the icon goes, and whether its corners say it is an opaque tile.
  const hash = icon?.startsWith("img:") ? icon.slice(4) : undefined;
  const iconUrl = hash !== undefined ? icons[hash] : undefined;
  const tile = useTile(face === "full" ? iconUrl : undefined);
  const full = face === "full" && icon !== undefined ? faceLayout(icon, tile, side, label !== undefined, iconUrl) : null;

  useEffect(() => {
    mounted.current = true;
    return () => {
      mounted.current = false;
      if (holdTimer.current !== null) window.clearTimeout(holdTimer.current);
      if (resetTimer.current !== null) window.clearTimeout(resetTimer.current);
    };
  }, []);

  function clearHold() {
    if (holdTimer.current !== null) window.clearTimeout(holdTimer.current);
    holdTimer.current = null;
  }

  function settleAfter(ms: number) {
    resetTimer.current = window.setTimeout(() => {
      if (mounted.current) setPhase("idle");
    }, ms);
  }

  function doPress() {
    if (actionId === undefined) return;
    const box = element.current?.getBoundingClientRect();
    setPhase("pending");
    onPress(actionId).then(
      () => {
        // The group opens even if this key is gone by now (its box was read at the touch).
        if (box) onPressed?.(box);
        if (!mounted.current) return;
        setPhase("ok");
        settleAfter(OK_MS);
      },
      (e: { code: string; message?: string }) => {
        onPressError(e);
        if (!mounted.current) return;
        setPhase("error");
        settleAfter(ERROR_MS);
      },
    );
  }

  function onDown(e: React.PointerEvent) {
    if (disabled || phaseRef.current === "pending") return;
    if (resetTimer.current !== null) window.clearTimeout(resetTimer.current);
    start.current = { x: e.clientX, y: e.clientY };
    onFeedback?.("touch");
    // Where the finger is on the key, for the light spot of the Glass theme (`--tx`, `--ty`; harmless elsewhere).
    const box = e.currentTarget.getBoundingClientRect();
    if (box.width > 0 && box.height > 0) {
      const el = e.currentTarget as HTMLElement;
      el.style.setProperty("--tx", `${Math.round(((e.clientX - box.left) / box.width) * 100)}%`);
      el.style.setProperty("--ty", `${Math.round(((e.clientY - box.top) / box.height) * 100)}%`);
    }
    if (actionId !== undefined && confirm) {
      setPhase("confirming");
      holdTimer.current = window.setTimeout(() => {
        holdTimer.current = null;
        start.current = null; // the release that follows must not press again
        onFeedback?.("confirm");
        doPress();
      }, CONFIRM_HOLD_MS);
    } else {
      setPhase("pressed");
    }
  }

  function cancel() {
    clearHold();
    start.current = null;
    if (phaseRef.current === "pressed" || phaseRef.current === "confirming") setPhase("idle");
  }

  function onMove(e: React.PointerEvent) {
    const s = start.current;
    if (s && Math.hypot(e.clientX - s.x, e.clientY - s.y) > TAP_SLOP_PX) cancel(); // it is a swipe
  }

  function onUp(e: React.PointerEvent) {
    const s = start.current;
    if (!s) {
      if (phaseRef.current === "pressed") setPhase("idle");
      return;
    }
    const moved = Math.hypot(e.clientX - s.x, e.clientY - s.y) > TAP_SLOP_PX;
    clearHold();
    start.current = null;
    if (moved || confirm) return setPhase("idle"); // released early: confirm is cancelled without sending
    if (actionId !== undefined) return doPress();
    setPhase("idle");
    const box = element.current?.getBoundingClientRect();
    if (box) onTap?.(box);
  }

  // The key is the touch target and the face at once: `side` x `side` px.
  // Colours, face and radius come from the theme through CSS variables; only the size and the key colour are set here.
  const stretch = fixedSide === undefined && metrics.cellW !== undefined && metrics.cellH !== undefined;
  const keyStyle = {
    width: stretch ? "100%" : side,
    height: stretch ? "100%" : side,
    ...(fixedSide !== undefined ? { "--side": `${fixedSide}px` } : {}),
    ...(coloured ? { ...colorVars(color), ...materialKeyVars(color as string) } : {}),
  } as CSSProperties;
  const classes = ["key", phase, coloured ? "colored" : "", disabled ? "disabled" : "", noData ? "no-data" : "", full ? "full" : "", full?.mode === "cover" ? "tile-face" : ""].filter(Boolean).join(" ");
  const labelPad = 0.07 * side;
  return (
    <div ref={element} className={classes} data-tile="" style={keyStyle} onPointerDown={onDown} onPointerMove={onMove} onPointerUp={onUp} onPointerCancel={cancel}>
      {full && full.mode === "cover" && iconUrl && <img className="key-cover" src={iconUrl} alt="" draggable={false} />}
      {full && full.mode === "contain" && iconUrl && (
        <div className="key-icon">
          <img src={iconUrl} width={full.iconPx} height={full.iconPx} alt="" draggable={false} style={{ objectFit: "contain", pointerEvents: "none" }} />
        </div>
      )}
      {full && full.mode === "center" && (
        <div className="key-icon">
          <Icon icon={icon} size={full.iconPx} />
        </div>
      )}
      {full && full.overlayLabel && label && (
        <div className="key-full-label" style={{ padding: `0 ${0.08 * side}px ${0.08 * side}px`, fontSize: Math.max(11, 0.13 * side) }}>
          <span>{label}</span>
        </div>
      )}
      {!full && icon && (
        <div className="key-icon" style={label ? { transform: `translateY(${-0.07 * side}px)` } : undefined}>
          <Icon icon={icon} size={Math.round(0.46 * side)} bitmapSize={Math.round((label ? 0.52 : 0.62) * side)} />
        </div>
      )}
      {!full && label && icon && (
        <div className="key-label" style={{ left: labelPad, right: labelPad, bottom: labelPad, fontSize: Math.max(11, 0.14 * side) }}>
          {label}
        </div>
      )}
      {label && !icon && (
        <div className="key-text" style={{ fontSize: 0.19 * side, padding: `0 ${labelPad}px` }}>
          {label}
        </div>
      )}
      {chevron && (
        <div className={full ? "key-chevron on-disc" : "key-chevron"} style={full ? discStyle(side, 0.14) : { top: 8, right: 8, fontSize: 14 }} aria-hidden="true">
          ›
        </div>
      )}
      {dots && (
        <div className={full ? "key-chevron on-disc" : "key-chevron"} style={full ? { ...discStyle(side, 0.12), letterSpacing: "0.05em" } : { top: 8, right: 8, fontSize: 14, letterSpacing: "0.05em" }} aria-hidden="true">
          •••
        </div>
      )}
      {liveDot && <span className="rec-dot key-live" style={{ top: 8, left: 8, width: 8, height: 8 }} aria-hidden="true" />}
      {phase === "confirming" && (
        <svg className="ring" width={side} height={side} viewBox={`0 0 ${side} ${side}`} aria-hidden="true">
          <rect x="1" y="1" width={side - 2} height={side - 2} pathLength="100" />
        </svg>
      )}
    </div>
  );
}
