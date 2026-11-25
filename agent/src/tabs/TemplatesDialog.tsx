import { useEffect, useState } from "react";
import { t, tn, type PageTemplate, type ValidationIssue } from "@decka/shared";
import { call } from "../ipc";
import Banner from "../ui/Banner";
import Button from "../ui/Button";
import Modal from "../ui/Modal";

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
    <Modal
      title={t("templates.title")}
      subtitle={t("templates.hint")}
      onClose={onClose}
      footer={<Button onClick={onClose}>{t("common.close")}</Button>}
    >
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
      {!templates && !error && <p>{t("common.loading")}</p>}
      <div className="template-list">
        {templates?.map((tpl) => (
          <div key={tpl.id} className="card template-card">
            <h3>{tpl.name}</h3>
            <p className="muted">{tpl.apps.join(", ")}</p>
            <p className="muted">{tn("grid.buttons_count", tpl.items.filter((i) => i.kind === "button").length)}</p>
            <Button variant="primary" disabled={busy} onClick={() => void add(tpl.id)}>
              {t("picker.add")}
            </Button>
          </div>
        ))}
      </div>
    </Modal>
  );
}
