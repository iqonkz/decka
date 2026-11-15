import { useEffect, useLayoutEffect, useState } from "react";
import { t } from "@decka/shared";
import Button from "../ui/Button";

export const TIPS_KEY = "decka.editor.tips.v1";

/** The three tips are shown once: the flag lives in this window's storage (blocked storage: they show again). */
export function tipsDone(storage: Pick<Storage, "getItem"> | undefined = safe()): boolean {
  try {
    return storage?.getItem(TIPS_KEY) === "1";
  } catch {
    return false;
  }
}
function markTipsDone() {
  try {
    safe()?.setItem(TIPS_KEY, "1");
  } catch {
    // nothing to remember it with
  }
}
function safe(): Storage | undefined {
  try {
    return window.localStorage;
  } catch {
    return undefined;
  }
}

const TIPS: { selector: string; text: () => string }[] = [
  { selector: ".editor-cell.empty", text: () => t("tips.empty_cell") },
  { selector: ".editor-item", text: () => t("tips.item") },
  { selector: ".editor-canvas", text: () => t("tips.canvas") },
];

/** Three tips in turn, each next to its element (DESIGN.md 4.7). `Next` / `Got it`; `Esc` hides them all. */
export default function EditorTips({ ready }: { ready: boolean }) {
  const [step, setStep] = useState(0);
  const [hidden, setHidden] = useState(() => tipsDone());
  const [pos, setPos] = useState<{ left: number; top: number } | null>(null);

  function finish() {
    markTipsDone();
    setHidden(true);
  }

  // Tips whose element is not on screen (no empty cell, no keys) are skipped.
  useLayoutEffect(() => {
    if (hidden || !ready) return;
    let i = step;
    while (i < TIPS.length && !document.querySelector(TIPS[i]!.selector)) i++;
    if (i >= TIPS.length) return finish();
    if (i !== step) return setStep(i);
    const box = document.querySelector(TIPS[i]!.selector)!.getBoundingClientRect();
    setPos({ left: Math.max(8, Math.min(box.left, window.innerWidth - 300)), top: Math.min(box.bottom + 8, window.innerHeight - 110) });
  }, [step, hidden, ready]); // eslint-disable-line react-hooks/exhaustive-deps

  useEffect(() => {
    if (hidden) return;
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") finish();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [hidden]);

  if (hidden || !ready || !pos || step >= TIPS.length) return null;
  const last = step === TIPS.length - 1;
  return (
    <div className="editor-tip" role="dialog" aria-label={t("tips.label")} style={{ left: pos.left, top: pos.top }}>
      <p>{TIPS[step]!.text()}</p>
      <div className="row">
        <small className="muted" style={{ flex: 1 }}>
          {t("tips.step", { n: step + 1, total: TIPS.length })}
        </small>
        <Button variant="primary" onClick={() => (last ? finish() : setStep(step + 1))}>
          {last ? t("tips.got_it") : t("tips.next")}
        </Button>
      </div>
    </div>
  );
}