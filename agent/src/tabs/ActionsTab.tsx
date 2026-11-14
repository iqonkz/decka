import { useCallback, useEffect, useMemo, useState } from "react";
import type { ActionsFile, LayoutFile, ValidationIssue } from "@decka/shared";
import { call } from "../ipc";
import Banner from "../ui/Banner";
import Button from "../ui/Button";
import ActionForm, { emptyDraft, fromDraft, toDraft, type Draft } from "./actionForm";

const NEW = "";
const ID_RE = /^[a-z0-9_-]{1,64}$/;

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
    const id = isNew ? draft.id.trim() : selected;
    if (isNew) {
      if (!ID_RE.test(id)) return setIssues([{ path: "/id", message: "ID must be 1 to 64 characters: a-z, 0-9, _ or -." }]);
      if (file.actions[id]) return setIssues([{ path: "/id", message: `An action with ID "${id}" already exists.` }]);
    }
    const built = fromDraft(draft);
    if ("error" in built) return setIssues([{ path: "/keys", message: built.error }]);

    const next: ActionsFile = { ...file, actions: { ...file.actions, [id]: built.action } };
    const result = await call("save_actions", { actions: next });
    if (result.ok) {
      setIssues([]);
      setMessage("Saved");
      await reload();
      setSelected(id);
      setDraft(toDraft(id, built.action));
    } else {
      setIssues(result.issues);
    }
  }

  async function test() {
    setTestResult("Running…");
    const result = await call("test_action", { actionId: selected });
    setTestResult(result.ok ? "OK" : (result.error ?? "Failed"));
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
  if (!file) return <p>Loading…</p>;

  const users = isNew ? [] : usedBy(selected);
  return (
    <div className="split">
      <div className="list-pane">
        <Button variant="primary" onClick={() => select(NEW)}>
          New action
        </Button>
        <ul className="plain-list">
          {list.map(([id, a]) => (
            <li key={id}>
              <button className={id === selected ? "list-item active" : "list-item"} onClick={() => select(id)}>
                <strong>{a.name}</strong>
                <span className="muted">
                  {a.type} · {id}
                </span>
              </button>
            </li>
          ))}
        </ul>
      </div>

      <div className="form-pane">
        <h2>{isNew ? "New action" : draft.name || selected}</h2>
        <ActionForm draft={draft} isNew={isNew} onChange={setDraft} />

        {issues.length > 0 && (
          <Banner kind="error">
            <ul>
              {issues.map((i, n) => (
                <li key={n}>
                  <code>{i.path}</code>: {i.message}
                </li>
              ))}
            </ul>
          </Banner>
        )}
        {message && <Banner kind="success">{message}</Banner>}

        <div className="row">
          <Button variant="primary" onClick={() => void save()}>
            Save
          </Button>
          <Button disabled={isNew} onClick={() => void test()}>
            Test
          </Button>
          <Button variant="danger" disabled={isNew} onClick={() => setConfirmDelete(true)}>
            Delete
          </Button>
          {testResult && <span className={testResult === "OK" ? "ok-text" : "warn-text"}>{testResult}</span>}
        </div>

        {confirmDelete && (
          <Banner kind="warning">
            <span>
              Delete {draft.name || selected}?{users.length > 0 ? ` It is used by these layout buttons: ${users.join(", ")}.` : ""}
            </span>
            <span className="row">
              <Button variant="danger" onClick={() => void remove()}>
                Delete
              </Button>
              <Button onClick={() => setConfirmDelete(false)}>Cancel</Button>
            </span>
          </Banner>
        )}
      </div>
    </div>
  );
}
