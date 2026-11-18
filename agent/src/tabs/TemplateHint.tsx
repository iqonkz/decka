import { useEffect, useState } from "react";
import { t, type AgentStatus, type LayoutFile, type PageTemplate } from "@decka/shared";
import { call, onStatus } from "../ipc";
import Banner from "../ui/Banner";
import Button from "../ui/Button";

/** The template for the program in front, when no page of the layout follows that program yet. */
export function suggestedTemplate(templates: PageTemplate[], layout: LayoutFile, app: string | null): PageTemplate | undefined {
  if (!app) return undefined;
  if (layout.pages.some((p) => p.apps?.includes(app) === true)) return undefined;
  return templates.find((tpl) => tpl.apps.includes(app));
}

/** "Outlook is in front and has no page. Add keys for it?" with a one-click button. */
export default function TemplateHint({ layout, onAdded }: { layout: LayoutFile; onAdded: () => void }) {
  const [templates, setTemplates] = useState<PageTemplate[]>([]);
  const [app, setApp] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let off: (() => void) | null = null;
    let disposed = false;
    call("get_templates").then(setTemplates, () => {});
    call("get_status").then((s: AgentStatus) => setApp(s.focusApp), () => {});
    onStatus((s) => setApp(s.focusApp)).then((unlisten) => {
      if (disposed) unlisten();
      else off = unlisten;
    });
    return () => {
      disposed = true;
      off?.();
    };
  }, []);

  const tpl = suggestedTemplate(templates, layout, app);
  if (!tpl || !app) return null;
  const add = async () => {
    setError(null);
    try {
      const result = await call("add_template", { templateId: tpl.id });
      if (result.ok) onAdded();
      else setError(result.issues[0]?.message ?? "");
    } catch (e) {
      setError(String(e));
    }
  };
  return (
    <Banner kind="info">
      {t("pageapps.suggest", { app })}{" "}
      <Button onClick={() => void add()}>{t("pageapps.suggest_add", { name: tpl.name })}</Button>
      {error && <span className="warn-text"> {error}</span>}
    </Banner>
  );
}
