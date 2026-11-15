import { useCallback, useEffect, useRef, useState, type CSSProperties } from "react";
import { useDeckEnv } from "./deckEnv";
import Icon from "./Icon";
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
  /** Called on a tap when there is no `actionId` (page navigation). */
  onTap?: () => void;
  /** Hold 800 ms instead of tapping. */
  confirm?: boolean;
  icon?: string;
  label?: string;
  color?: string;
  /** Key side in px. Without it the key is one grid cell. */
  side?: number;
  /** Small `›` in the corner: the key opens another page. */
  chevron?: boolean;
}

/** One key with the shared press states: pressed, confirming, pending, ok, error. Used by buttons and widgets. */
export default function Key({ actionId, onTap, confirm, icon, label, color, side: fixedSide, chevron }: KeyProps) {
  const { online, onPress, onPressError, onFeedback } = useDeckEnv();
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
    setPhase("pending");
    onPress(actionId).then(
      () => {
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
    onTap?.();
  }

  // The key is the touch target and the face at once: `side` x `side` px.
  // Colours, face and radius come from the theme through CSS variables; only the size and the key colour are set here.
  const keyStyle = {
    width: side,
    height: side,
    ...(fixedSide !== undefined ? { "--side": `${fixedSide}px` } : {}),
    ...(coloured ? { ...colorVars(color), ...materialKeyVars(color as string) } : {}),
  } as CSSProperties;
  const classes = ["key", phase, coloured ? "colored" : "", disabled ? "disabled" : ""].filter(Boolean).join(" ");
  const labelPad = 0.07 * side;
  return (
    <div className={classes} style={keyStyle} onPointerDown={onDown} onPointerMove={onMove} onPointerUp={onUp} onPointerCancel={cancel}>
      {icon && (
        <div className="key-icon" style={label ? { transform: `translateY(${-0.07 * side}px)` } : undefined}>
          <Icon icon={icon} size={Math.round((icon.startsWith("img:") ? 0.62 : 0.46) * side)} />
        </div>
      )}
      {label && icon && (
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
        <div className="key-chevron" style={{ top: 0.04 * side, right: 0.07 * side, fontSize: 0.14 * side }} aria-hidden="true">
          ›
        </div>
      )}
      {phase === "confirming" && (
        <svg className="ring" width={side} height={side} viewBox={`0 0 ${side} ${side}`} aria-hidden="true">
          <rect x="1" y="1" width={side - 2} height={side - 2} pathLength="100" />
        </svg>
      )}
    </div>
  );
}
