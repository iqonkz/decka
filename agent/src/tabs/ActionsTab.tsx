import { useCallback, useEffect, useMemo, useState } from "react";
import { t, tn, type ActionsFile, type LayoutFile, type ValidationIssue } from "@decka/shared";
import { call } from "../ipc";
import Banner from "../ui/Banner";
import Button from "../ui/Button";
import { makeActionId } from "../editor/addButton";
import { describeAction } from "../editor/describeAction";
import ActionForm, { emptyDraft, fromDraft, toDraft, type Draft } from "./actionForm";

const NEW = "";

export default function ActionsTab() {
  const [file, setFile] = useState<ActionsFile | null>(null);
  const [layout, setLayout] = useState<LayoutFile | null>(null);
  const [selected, setSelected] = useState<string>(NEW);
  const [draft, setDraft] = useState<Draft>(emptyDraft());
  const [issues, setIssues] = useState<ValidationIssue[]>([]);
  const [message, setMessage] = useState<string | null>(null);
  const [testResult, setTestResult] = useState<string | null>(null);
  const [confirmDelete, setConfirmDelete] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const reload = useCallback(async () => {
    setFile(await call("get_actions"));
    setLayout(await call("get_layout"));
  }, []);

  useEffect(() => {
    reload().catch((e) => setError(String(e)));
  }, [reload]);

  const list = useMemo(() => Object.entries(file?.actions ?? {}).sort(([, a], [, b]) => a.name.localeCompare(b.name)), [file]);
  const isNew = selected === NEW;

  function select(id: string) {
    setSelected(id);
    setIssues([]);
    setMessage(null);
    setTestResult(null);
    setConfirmDelete(false);
    const action = file?.actions[id];
    setDraft(action ? toDraft(id, action) : emptyDraft());
  }

  /** Ids of layout buttons that run this action. */
  const usedBy = (id: string): string[] =>
    (layout?.pages ?? []).flatMap((p) => p.items.filter((i) => i.kind === "button" && i.actionId === id).map((i) => i.id));

  async function save() {
    if (!file) return;
    setMessage(null);
    setTestResult(null);
    if (draft.name.trim() === "") return setIssues([{ path: "/name", message: t("actions.need_name") }]);
    // The ID is never typed: a new action gets one made from its name.
    const id = isNew ? makeActionId(draft.name, file.actions) : selected;
    const built = fromDraft(draft);
    if ("error" in built) return setIssues([{ path: "/keys", message: built.error }]);

    const next: ActionsFile = { ...file, actions: { ...file.actions, [id]: built.action } };
    const result = await call("save_actions", { actions: next });
    if (result.ok) {
      setIssues([]);
      setMessage(t("common.saved"));
      await reload();
      setSelected(id);
      setDraft(toDraft(id, built.action));
    } else {
      setIssues(result.issues);
    }
  }

  async function test() {
    setTestResult(t("actions.running"));
    const result = await call("test_action", { actionId: selected });
    setTestResult(result.ok ? t("common.ok") : (result.error ?? t("actions.failed")));
  }

  async function remove() {
    if (!file || isNew) return;
    const { [selected]: _gone, ...rest } = file.actions;
    const result = await call("save_actions", { actions: { ...file, actions: rest } });
    if (result.ok) {
      setConfirmDelete(false);
      setSelected(NEW);
      setDraft(emptyDraft());
      await reload();
    } else {
      setIssues(result.issues);
    }
  }

  if (error) return <Banner kind="error">{error}</Banner>;
  if (!file) return <p>{t("common.loading")}</p>;

  const users = isNew ? [] : usedBy(selected);
  return (
    <div className="split">
      <div className="list-pane">
        <Button variant="primary" onClick={() => select(NEW)}>
          {t("actions.new")}
        </Button>
        <ul className="plain-list">
          {list.map(([id, a]) => (
            <li key={id}>
              <button className={id === selected ? "list-item active" : "list-item"} onClick={() => select(id)}>
                <strong>{a.name}</strong>
                <span className="muted">{describeAction(a)}</span>
              </button>
            </li>
          ))}
        </ul>
      </div>

      <div className="form-pane">
        <h2>{isNew ? t("actions.new") : draft.name || t("actions.action")}</h2>
        <ActionForm draft={draft} isNew={isNew} onChange={setDraft} />

        {issues.length > 0 && (
          <Banner kind="error">
            <ul>
              {issues.map((i, n) => (
                <li key={n}>{i.message}</li>
              ))}
            </ul>
          </Banner>
        )}
        {message && <Banner kind="success">{message}</Banner>}

        <div className="row">
          <Button variant="primary" onClick={() => void save()}>
            {t("common.save")}
          </Button>
          <Button disabled={isNew} onClick={() => void test()}>
            {t("actions.test")}
          </Button>
          <Button variant="danger" disabled={isNew} onClick={() => setConfirmDelete(true)}>
            {t("common.delete")}
          </Button>
          {testResult && <span className={testResult === t("common.ok") ? "ok-text" : "warn-text"}>{testResult}</span>}
        </div>

        {confirmDelete && (
          <Banner kind="warning">
            <span>
              {t("actions.delete_confirm", { name: draft.name || t("actions.this_action") })}
              {users.length > 0 ? ` ${tn("actions.used_by", users.length)}` : ""}
            </span>
            <span className="row">
              <Button variant="danger" onClick={() => void remove()}>
                {t("common.delete")}
              </Button>
              <Button onClick={() => setConfirmDelete(false)}>{t("common.cancel")}</Button>
            </span>
          </Banner>
        )}
      </div>
    </div>
  );
}
