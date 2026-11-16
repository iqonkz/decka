import { useMemo, useState } from "react";
import { SEQUENCE_LIMITS, t, validateActions, type Action, type ActionsFile, type SequenceStep } from "@decka/shared";
import Button from "../ui/Button";
import PhIcon from "../ui/PhIcon";

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
  // The actions a step can run: not the sequences (no nesting).
  const choices = useMemo(
    () => Object.entries(actions.actions).filter(([, a]) => a.type !== "sequence").sort(([, a], [, b]) => a.name.localeCompare(b.name)),
    [actions],
  );

  const draft: Action = { name: name.trim() || t("seq.default_name"), type: "sequence", steps };
  const issues = useMemo(() => (steps.length === 0 ? [] : validateActions({ version: 1, actions: { ...actions.actions, "draft-seq": draft } }).filter((i) => i.path.startsWith("/actions/draft-seq"))), [actions, steps, name]);
  const totalMs = steps.reduce((sum, s) => sum + ("delayMs" in s ? s.delayMs : 0), 0);

  const set = (i: number, step: SequenceStep) => setSteps((list) => list.map((s, n) => (n === i ? step : s)));
  const move = (i: number, by: number) =>
    setSteps((list) => {
      const to = i + by;
      if (to < 0 || to >= list.length) return list;
      const copy = [...list];
      const [item] = copy.splice(i, 1);
      copy.splice(to, 0, item!);
      return copy;
    });
  const full = steps.length >= SEQUENCE_LIMITS.steps;

  return (
    <div className="dialog-backdrop" onPointerDown={onClose}>
      <div className="dialog sequence-dialog" role="dialog" aria-label={t("seq.title")} onPointerDown={(e) => e.stopPropagation()}>
        <h2>{t("seq.title")}</h2>
        <label className="field-row">
          <span>{t("seq.name")}</span>
          <input value={name} maxLength={60} onChange={(e) => setName(e.target.value)} />
        </label>
        <p className="muted">{t("seq.help")}</p>
        {choices.length === 0 && <p className="warn-text">{t("seq.no_actions")}</p>}
        <ol className="seq-steps">
          {steps.map((step, i) => (
            <li key={i} className="seq-step">
              {"delayMs" in step ? (
                <>
                  <PhIcon name="timer" size={18} />
                  <span>{t("seq.wait")}</span>
                  <input type="number" min={SEQUENCE_LIMITS.delayMin} max={SEQUENCE_LIMITS.delayMax} step={50} value={step.delayMs} onChange={(e) => set(i, { delayMs: Number(e.target.value) })} />
                  <span className="muted">{t("seq.ms")}</span>
                </>
              ) : (
                <select value={step.actionId} aria-label={t("seq.pick")} onChange={(e) => set(i, { actionId: e.target.value })}>
                  <option value="">{t("seq.pick")}</option>
                  {choices.map(([id, a]) => <option key={id} value={id}>{a.name}</option>)}
                </select>
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
        <div className="row">
          <Button variant="primary" disabled={steps.length === 0 || steps.some((s) => "actionId" in s && s.actionId === "") || issues.length > 0} onClick={() => onSave(draft.name, steps)}>{t("seq.save")}</Button>
          <Button onClick={onClose}>{t("common.cancel")}</Button>
        </div>
      </div>
    </div>
  );
}
