import { useEffect, useState } from "react";
import type { PageTemplate, ValidationIssue } from "@decka/shared";
import { call } from "../ipc";
import Banner from "../ui/Banner";
import Button from "../ui/Button";

interface Props {
  onClose: () => void;
  /** Called after a template was added and saved; the layout and actions must be reloaded. */
  onAdded: () => void;
}

/** Cards of the built-in context page templates (DATA_MODEL.md 7). */
export default function TemplatesDialog({ onClose, onAdded }: Props) {
  const [templates, setTemplates] = useState<PageTemplate[] | null>(null);
  const [issues, setIssues] = useState<ValidationIssue[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    call("get_templates").then(setTemplates, (e) => setError(String(e)));
  }, []);

  async function add(id: string) {
    setBusy(true);
    setIssues([]);
    try {
      const result = await call("add_template", { templateId: id });
      if (result.ok) onAdded();
      else setIssues(result.issues);
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  }

  return (
    <div className="dialog-backdrop">
      <div className="dialog" role="dialog" aria-label="Add page from template">
        <h2>Add page from template</h2>
        <p className="muted">Adds a page with ready-made buttons and actions. Unsaved layout changes are discarded.</p>
        {error && <Banner kind="error">{error}</Banner>}
        {issues.length > 0 && (
          <Banner kind="error">
            <ul>
              {issues.map((i, n) => (
                <li key={n}>{i.message}</li>
              ))}
            </ul>
          </Banner>
        )}
        {!templates && !error && <p>Loading…</p>}
        <div className="template-list">
          {templates?.map((t) => (
            <div key={t.id} className="card template-card">
              <h3>{t.name}</h3>
              <p className="muted">{t.apps.join(", ")}</p>
              <p className="muted">{t.items.filter((i) => i.kind === "button").length} buttons</p>
              <Button variant="primary" disabled={busy} onClick={() => void add(t.id)}>
                Add
              </Button>
            </div>
          ))}
        </div>
        <div className="row">
          <Button onClick={onClose}>Close</Button>
        </div>
      </div>
    </div>
  );
}
