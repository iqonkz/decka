import { useCallback, useEffect, useState } from "react";
import { t, type MemoryFact } from "@decka/shared";
import { call } from "../ipc";
import Banner from "../ui/Banner";
import Button from "../ui/Button";
import ConfirmButton from "../ui/ConfirmButton";
import PhIcon from "../ui/PhIcon";

const MAX_FACTS = 200;

/** `More -> Voice -> Memory` (DATA_MODEL.md 13.7): what the assistant was asked to remember; edit in the row, delete, forget all. */
export default function MemoryBlock() {
  const [facts, setFacts] = useState<MemoryFact[]>([]);
  const [editing, setEditing] = useState<{ id: string; text: string } | null>(null);
  const [error, setError] = useState<string | null>(null);
  const load = useCallback(() => call("memory_get").then(setFacts, (e) => setError(String(e))), []);
  useEffect(() => {
    void load();
  }, [load]);
  const run = async (p: Promise<unknown>) => {
    setError(null);
    try {
      await p;
    } catch (e) {
      setError(String(e));
    }
    await load();
  };
  const commit = () => {
    const e = editing;
    setEditing(null);
    const old = facts.find((f) => f.id === e?.id)?.text;
    if (e && e.text.trim() !== "" && e.text.trim() !== old) void run(call("memory_update", { id: e.id, text: e.text.trim() }));
  };
  return (
    <section className="card" id="memory">
      <div className="row">
        <h3>{t("mem.title")}</h3>
        <span className="grow" />
        <span className="muted" aria-label={t("mem.title")}>{`${facts.length} / ${MAX_FACTS}`}</span>
      </div>
      {error && <Banner kind="error">{error}</Banner>}
      {facts.length === 0 && <p className="muted">{t("mem.empty")}</p>}
      {facts.map((f) => (
        <div className="row rem-line" key={f.id}>
          {editing?.id === f.id ? (
            <input autoFocus className="grow" maxLength={200} aria-label={t("mem.title")} value={editing.text} onChange={(e) => setEditing({ id: f.id, text: e.target.value })} onBlur={commit} onKeyDown={(e) => (e.key === "Enter" ? commit() : e.key === "Escape" ? setEditing(null) : undefined)} />
          ) : (
            <button type="button" className="link rem-edit grow" onClick={() => setEditing({ id: f.id, text: f.text })}>
              {f.text}
            </button>
          )}
          <Button className="btn-icon" aria-label={t("common.delete")} title={t("common.delete")} onClick={() => void run(call("memory_delete", { id: f.id }))}>
            <PhIcon name="trash" size={16} />
          </Button>
        </div>
      ))}
      <p className="muted">{t("mem.hint")}</p>
      {facts.length > 0 && <ConfirmButton label={t("mem.forget_all")} question={t("mem.confirm_clear")} onConfirm={() => void run(call("memory_clear"))} />}
    </section>
  );
}