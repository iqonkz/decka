import { useCallback, useEffect, useRef, useState, type CSSProperties } from "react";
import type { ButtonItem } from "@decka/shared";
import { press, setPage } from "../app/controller";
import type { ConnFailure } from "../net/connection";
import { getState, pushToast, useAppState } from "../store/store";
import Icon from "./Icon";
import { keyColors, pressErrorTitle, resolveLook } from "./look";
import { useKeySize } from "./useKeySize";

const TAP_SLOP_PX = 20;
const CONFIRM_HOLD_MS = 800;
const OK_MS = 300;
const ERROR_MS = 1500;
const ERROR_TOAST_MS = 4000;

type Phase = "idle" | "pressed" | "confirming" | "pending" | "ok" | "error";

export default function Button({ item }: { item: ButtonItem }) {
  const vars = useAppState((s) => s.state?.vars);
  const online = useAppState((s) => s.conn === "online");
  const look = resolveLook(item, vars);
  const disabled = item.actionId !== undefined && !online;

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
  const [box, side] = useKeySize();
  const colors = keyColors(look.color);

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
    const actionId = item.actionId;
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
    if (item.actionId !== undefined && item.confirm) {
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
    if (moved || item.confirm) return setPhase("idle"); // released early: confirm is cancelled without sending
    if (item.actionId !== undefined) return doPress();
    setPhase("idle");
    if (item.navigate !== undefined) {
      const index = getState().state?.layout.pages.findIndex((p) => p.id === item.navigate) ?? -1;
      if (index >= 0) setPage(index);
    }
  }

  // The whole cell is the touch target; the key inside it is what the eye sees.
  const classes = ["btn", disabled ? "disabled" : ""].filter(Boolean).join(" ");
  const keyStyle: CSSProperties = {
    width: side,
    height: side,
    borderRadius: 0.22 * side,
    background: phase === "pressed" ? colors.pressed : colors.background,
    borderColor: colors.border,
    color: colors.ink,
  };
  return (
    <div ref={box} className={classes} onPointerDown={onDown} onPointerMove={onMove} onPointerUp={onUp} onPointerCancel={cancel}>
      <div className={`key ${phase}`} style={keyStyle}>
        {look.icon ? (
          <Icon icon={look.icon} size={Math.round(0.5 * side)} />
        ) : (
          <div className="key-text" style={{ fontSize: 0.2 * side }}>
            {look.label ?? ""}
          </div>
        )}
        {phase === "confirming" && (
          <svg className="ring" width={side} height={side} viewBox={`0 0 ${side} ${side}`} aria-hidden="true">
            <rect x="1" y="1" width={side - 2} height={side - 2} rx={0.22 * side} pathLength="100" />
          </svg>
        )}
      </div>
      {look.icon && look.label && (
        <div className="key-label" style={{ marginTop: 0.06 * side, fontSize: Math.max(11, 0.16 * side) }}>
          {look.label}
        </div>
      )}
    </div>
  );
}
