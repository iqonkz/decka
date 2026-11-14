import { useCallback, useEffect, useRef, useState, type CSSProperties } from "react";
import { press } from "../app/controller";
import type { ConnFailure } from "../net/connection";
import { pushToast, useAppState } from "../store/store";
import Icon from "./Icon";
import { keyColors, pressErrorTitle } from "./look";
import { useKeySize } from "./useKeySize";

const TAP_SLOP_PX = 20;
const CONFIRM_HOLD_MS = 800;
const OK_MS = 300;
const ERROR_MS = 1500;
const ERROR_TOAST_MS = 4000;

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
  /** Fixed key side in px (the touch target is the key itself). Without it the key fills its cell (70%). */
  side?: number;
}

/** One key with the shared press states: pressed, confirming, pending, ok, error. Used by buttons and widgets. */
export default function Key({ actionId, onTap, confirm, icon, label, color, side: fixedSide }: KeyProps) {
  const online = useAppState((s) => s.conn === "online");
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
  const [box, cellSide] = useKeySize();
  const side = fixedSide ?? cellSide;
  const colors = keyColors(color);

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
    press(actionId).then(
      () => {
        if (!mounted.current) return;
        setPhase("ok");
        settleAfter(OK_MS);
      },
      (e: ConnFailure) => {
        pushToast({ level: "error", title: pressErrorTitle(e), ms: ERROR_TOAST_MS });
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
    if (actionId !== undefined && confirm) {
      setPhase("confirming");
      holdTimer.current = window.setTimeout(() => {
        holdTimer.current = null;
        start.current = null; // the release that follows must not press again
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

  // In a cell the whole cell is the touch target; the key inside it is what the eye sees.
  const classes = ["btn", disabled ? "disabled" : ""].filter(Boolean).join(" ");
  const wrapStyle: CSSProperties | undefined = fixedSide === undefined ? undefined : { width: fixedSide, height: fixedSide, flex: "none" };
  const keyStyle: CSSProperties = {
    width: side,
    height: side,
    borderRadius: 0.22 * side,
    background: phase === "pressed" ? colors.pressed : colors.background,
    borderColor: colors.border,
    color: colors.ink,
  };
  return (
    <div ref={box} className={classes} style={wrapStyle} onPointerDown={onDown} onPointerMove={onMove} onPointerUp={onUp} onPointerCancel={cancel}>
      <div className={`key ${phase}`} style={keyStyle}>
        {icon ? (
          <Icon icon={icon} size={Math.round(0.5 * side)} />
        ) : (
          <div className="key-text" style={{ fontSize: 0.2 * side }}>
            {label ?? ""}
          </div>
        )}
        {phase === "confirming" && (
          <svg className="ring" width={side} height={side} viewBox={`0 0 ${side} ${side}`} aria-hidden="true">
            <rect x="1" y="1" width={side - 2} height={side - 2} rx={0.22 * side} pathLength="100" />
          </svg>
        )}
      </div>
      {icon && label && fixedSide === undefined && (
        <div className="key-label" style={{ marginTop: 0.06 * side, fontSize: Math.max(11, 0.16 * side) }}>
          {label}
        </div>
      )}
    </div>
  );
}
