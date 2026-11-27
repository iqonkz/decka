import { useEffect, useReducer, useRef, useState } from "react";
import type { TimerItem } from "@decka/shared";
import { useDeckEnv } from "../deckEnv";
import { DEFAULT_PRESETS, formatRemaining, timerInit, timerReducer } from "./timerState";

const HOLD_MS = 800;
const TICK_MS = 250;
const BUZZ_COUNT = 3;
const BUZZ_GAP_MS = 220;
const RING_SIZE = 100;
const RING_STROKE = 4;
const RING_R = (RING_SIZE - RING_STROKE) / 2;
const RING_C = 2 * Math.PI * RING_R;

/** The timer (DATA_MODEL.md 6.7, DESIGN.md 4.16): 4 px accent progress ring, main-size digits, 28 px preset pills. */
export default function Timer({ item }: { item: TimerItem }) {
  const { onFeedback, onTimerDone } = useDeckEnv();
  const presets = item.presets ?? DEFAULT_PRESETS;
  const [state, dispatch] = useReducer(timerReducer, presets[0] ?? 25, timerInit);
  const [flashing, setFlashing] = useState(false);

  // The clock ticks only while it runs.
  useEffect(() => {
    if (!state.running) return;
    const id = window.setInterval(() => dispatch({ type: "tick", now: Date.now() }), TICK_MS);
    return () => window.clearInterval(id);
  }, [state.running]);

  // The end: a buzz three times, the host's toast and wake-up, the frame flashes three times.
  const wasDone = useRef(false);
  useEffect(() => {
    if (state.done && !wasDone.current) {
      onTimerDone?.();
      setFlashing(true);
      const timers = Array.from({ length: BUZZ_COUNT }, (_, i) => window.setTimeout(() => onFeedback?.("confirm"), i * BUZZ_GAP_MS));
      const stop = window.setTimeout(() => setFlashing(false), BUZZ_COUNT * 1000);
      timers.push(stop);
      wasDone.current = true;
      return () => timers.forEach((id) => window.clearTimeout(id));
    }
    if (!state.done) wasDone.current = false;
    return undefined;
  }, [state.done, onFeedback, onTimerDone]);

  // A hold resets, a short press toggles.
  const hold = useRef<number | null>(null);
  const held = useRef(false);
  const onDown = () => {
    held.current = false;
    hold.current = window.setTimeout(() => {
      hold.current = null;
      held.current = true;
      onFeedback?.("confirm");
      dispatch({ type: "reset" });
    }, HOLD_MS);
  };
  const clearHold = () => {
    if (hold.current !== null) window.clearTimeout(hold.current);
    hold.current = null;
  };
  const onUp = () => {
    const wasHold = held.current;
    clearHold();
    if (!wasHold) dispatch({ type: "toggle", now: Date.now() });
  };
  useEffect(() => clearHold, []);

  const totalMs = Math.max(1, state.minutes * 60_000);
  const ratio = Math.min(1, Math.max(0, state.remainingMs / totalMs));
  const shown = formatRemaining(state.remainingMs);

  return (
    <div className={["timer", state.running ? "running" : "", state.done ? "done" : "", flashing ? "flash" : "", item.h >= 2 ? "tall" : ""].filter(Boolean).join(" ")}>
      <div className="timer-main" onPointerDown={onDown} onPointerUp={onUp} onPointerCancel={clearHold} onPointerLeave={clearHold}>
        <div className="timer-ring-wrap">
          <svg className="timer-ring" viewBox={`0 0 ${RING_SIZE} ${RING_SIZE}`} aria-hidden="true">
            <circle cx={RING_SIZE / 2} cy={RING_SIZE / 2} r={RING_R} fill="none" stroke="rgba(127, 127, 127, 0.24)" strokeWidth={RING_STROKE} />
            <circle
              cx={RING_SIZE / 2}
              cy={RING_SIZE / 2}
              r={RING_R}
              fill="none"
              stroke={state.done ? "var(--error, #ef4444)" : "var(--accent, #7c5cff)"}
              strokeWidth={RING_STROKE}
              strokeLinecap="round"
              strokeDasharray={`${RING_C * ratio} ${RING_C}`}
              transform={`rotate(-90 ${RING_SIZE / 2} ${RING_SIZE / 2})`}
            />
          </svg>
          <div className={`timer-time widget-figure${shown.length > 5 ? " long" : ""}`}>{shown}</div>
        </div>
      </div>
      <div className="timer-presets w-m">
        {presets.map((m) => (
          <button key={m} type="button" className={m === state.minutes ? "on" : ""} onClick={() => dispatch({ type: "preset", minutes: m })}>
            {m}
          </button>
        ))}
      </div>
    </div>
  );
}
