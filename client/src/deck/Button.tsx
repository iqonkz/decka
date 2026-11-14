import { useCallback, useEffect, useRef, useState } from "react";
import type { ButtonItem } from "@decka/shared";
import { press, setPage } from "../app/controller";
import type { ConnFailure } from "../net/connection";
import { getState, pushToast, useAppState } from "../store/store";
import Icon from "./Icon";
import { pressErrorTitle, resolveLook } from "./look";

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
  const box = useRef<HTMLDivElement>(null);
  const [iconSize, setIconSize] = useState(32);

  useEffect(() => {
    mounted.current = true;
    return () => {
      mounted.current = false;
      if (holdTimer.current !== null) window.clearTimeout(holdTimer.current);
      if (resetTimer.current !== null) window.clearTimeout(resetTimer.current);
    };
  }, []);

  // Icon size = 38% of the tile's smaller side.
  useEffect(() => {
    const el = box.current;
    if (!el) return;
    const measure = () => setIconSize(Math.round(Math.min(el.clientWidth, el.clientHeight) * 0.38));
    measure();
    const observer = new ResizeObserver(measure);
    observer.observe(el);
    return () => observer.disconnect();
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

  const classes = ["btn", phase, disabled ? "disabled" : ""].filter(Boolean).join(" ");
  return (
    <div
      ref={box}
      className={classes}
      style={{ background: look.color ?? "#1e1e24" }}
      onPointerDown={onDown}
      onPointerMove={onMove}
      onPointerUp={onUp}
      onPointerCancel={cancel}
    >
      {look.icon ? (
        <>
          <div className="btn-icon">
            <Icon icon={look.icon} size={iconSize} />
          </div>
          {look.label && <div className="btn-label">{look.label}</div>}
        </>
      ) : (
        <div className="btn-label solo">{look.label ?? ""}</div>
      )}
      {phase === "confirming" && (
        <svg className="ring" viewBox="0 0 36 36" aria-hidden="true">
          <circle cx="18" cy="18" r="16" />
        </svg>
      )}
    </div>
  );
}
