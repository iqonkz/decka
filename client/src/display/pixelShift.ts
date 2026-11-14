import { useEffect, useState } from "react";

const AMPLITUDE_PX = 4;
const STEPS = 16;
const STEP_MS = 60_000;

/** Offset for a step; one lap takes 16 steps. Never farther than 4 px from the centre. */
export function shiftOffset(step: number): { x: number; y: number } {
  const x = Math.round(AMPLITUDE_PX * Math.sin((2 * Math.PI * step) / STEPS));
  const y = Math.round(AMPLITUDE_PX * Math.sin((4 * Math.PI * step) / STEPS));
  // Math.round(-0) is -0; keep plain zeros so offsets compare equal.
  return { x: x === 0 ? 0 : x, y: y === 0 ? 0 : y };
}

/** Current offset; the step grows by one every 60 s from app start. `enabled = false` means no shift. */
export function usePixelShift(enabled: boolean): { x: number; y: number } {
  const [step, setStep] = useState(0);
  useEffect(() => {
    if (!enabled) return;
    const timer = window.setInterval(() => setStep((s) => s + 1), STEP_MS);
    return () => window.clearInterval(timer);
  }, [enabled]);
  return enabled ? shiftOffset(step) : { x: 0, y: 0 };
}
