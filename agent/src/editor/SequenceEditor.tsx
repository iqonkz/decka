import { useEffect, useMemo, useRef, useState } from "react";
import { dropIndex, moveStep } from "./seqOps";
import { SEQUENCE_LIMITS, t, validateActions, type Action, type ActionsFile, type SequenceStep } from "@decka/shared";
import Button from "../ui/Button";
import Modal from "../ui/Modal";
import PhIcon from "../ui/PhIcon";
import Select from "../ui/Select";

export interface SequenceProps {
  actions: ActionsFile;
  onSave: (name: string, steps: SequenceStep[]) => void;
  onClose: () => void;
}

const DEFAULT_WAIT_MS = 500;

/** The editor of a sequence (DESIGN.md 4.8): a name and a list of steps, each an action of yours or a pause. */
export default function SequenceEditor({ actions, onSave, onClose }: SequenceProps) {
  const [name, setName] = useState(t("seq.default_name"));
  const [steps, setSteps] = useState<SequenceStep[]>([]);
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") onClose();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [onClose]);
  // The actions a step can run: not the sequences (no nesting).
  const choices = useMemo(
    () => Object.entries(actions.actions).filter(([, a]) => a.type !== "sequence").sort(([, a], [, b]) => a.name.localeCompare(b.name)),
    [actions],
  );

  const draft: Action = { name: name.trim() || t("seq.default_name"), type: "sequence", steps };
  const issues = useMemo(() => (steps.length === 0 ? [] : validateActions({ version: 1, actions: { ...actions.actions, "draft-seq": draft } }).filter((i) => i.path.startsWith("/actions/draft-seq"))), [actions, steps, name]);
  const totalMs = steps.reduce((sum, s) => sum + ("delayMs" in s ? s.delayMs : 0), 0);

  const set = (i: number, step: SequenceStep) => setSteps((list) => list.map((s, n) => (n === i ? step : s)));
  const move = (i: number, by: number) => setSteps((l) => moveStep(l, i, i + by) as SequenceStep[]);
  const full = steps.length >= SEQUENCE_LIMITS.steps;

  // Drag a step by its handle (pointer events: HTML5 drag and drop does not work in the Tauri window, D-38).
  const list = useRef<HTMLOListElement>(null);
  const [drag, setDrag] = useState<{ from: number; over: number } | null>(null);
  const overAt = (y: number): number => {
    const rows = Array.from(list.current?.children ?? []) as HTMLElement[];
    return dropIndex(rows.map((r) => r.getBoundingClientRect().top + r.offsetHeight / 2), y);
  };
  const startDrag = (e: React.PointerEvent, from: number) => {
    if (e.button !== 0) return;
    e.preventDefault();
    e.currentTarget.setPointerCapture(e.pointerId);
    setDrag({ from, over: from });
  };
  const dragMove = (e: React.PointerEvent) => drag && setDrag({ ...drag, over: overAt(e.clientY) });
  const dragEnd = (e: React.PointerEvent) => {
    if (!drag) return;
    const to = overAt(e.clientY);
    setSteps((l) => moveStep(l, drag.from, to) as SequenceStep[]);
    setDrag(null);
  };

  return (
    <Modal
      title={t("seq.title")}
      subtitle={t("seq.help")}
      onClose={onClose}
      className="sequence-dialog"
      footer={
        <>
          <Button onClick={onClose}>{t("common.cancel")}</Button>
          <Button variant="primary" disabled={steps.length === 0 || steps.some((s) => "actionId" in s && s.actionId === "") || issues.length > 0} onClick={() => onSave(draft.name, steps)}>{t("seq.save")}</Button>
        </>
      }
    >
      <label className="field-row">
        <span>{t("seq.name")}</span>
        <input value={name} maxLength={60} onChange={(e) => setName(e.target.value)} />
      </label>
      {choices.length === 0 && <p className="warn-text">{t("seq.no_actions")}</p>}
      <ol className="seq-steps" ref={list}>
        {steps.map((step, i) => (
          <li key={i} className={`seq-step${drag?.from === i ? " dragging" : ""}${drag && drag.over === i && drag.from !== i ? (drag.over > drag.from ? " drop-after" : " drop-before") : ""}`}>
            <span
              className="seq-handle"
              role="button"
              aria-label={t("seq.drag")}
              title={t("seq.drag")}
              onPointerDown={(e) => startDrag(e, i)}
              onPointerMove={dragMove}
              onPointerUp={dragEnd}
              onPointerCancel={() => setDrag(null)}
            >
              <PhIcon name="dots-six-vertical" size={16} />
            </span>
            {"delayMs" in step ? (
              <>
                <PhIcon name="timer" size={18} />
                <span>{t("seq.wait")}</span>
                <input type="number" min={SEQUENCE_LIMITS.delayMin} max={SEQUENCE_LIMITS.delayMax} step={50} value={step.delayMs} onChange={(e) => set(i, { delayMs: Number(e.target.value) })} />
                <span className="muted">{t("seq.ms")}</span>
              </>
            ) : (
              <Select
                aria-label={t("seq.pick")}
                value={step.actionId}
                placeholder={t("seq.pick")}
                options={choices.map(([id, a]) => ({ value: id, label: a.name }))}
                onChange={(val) => set(i, { actionId: val })}
              />
            )}
            <button type="button" className="icon-btn" aria-label={t("seq.up")} disabled={i === 0} onClick={() => move(i, -1)}><PhIcon name="arrow-up" size={14} /></button>
            <button type="button" className="icon-btn" aria-label={t("seq.down")} disabled={i === steps.length - 1} onClick={() => move(i, 1)}><PhIcon name="arrow-down" size={14} /></button>
            <button type="button" className="icon-btn" aria-label={t("seq.remove")} onClick={() => setSteps((list) => list.filter((_, n) => n !== i))}><PhIcon name="x" size={14} /></button>
          </li>
        ))}
      </ol>
      <div className="row">
        <Button disabled={full || choices.length === 0} onClick={() => setSteps((l) => [...l, { actionId: "" }])}>{t("seq.add_step")}</Button>
        <Button disabled={full} onClick={() => setSteps((l) => [...l, { delayMs: DEFAULT_WAIT_MS }])}>{t("seq.wait")}</Button>
        <span className="grow" />
        <span className="muted">{t("seq.summary", { steps: steps.length, seconds: Math.round(totalMs / 100) / 10 })}</span>
      </div>
      {issues.length > 0 && <ul className="warn-text">{issues.map((i, n) => <li key={n}>{i.message}</li>)}</ul>}
    </Modal>
  );
}
