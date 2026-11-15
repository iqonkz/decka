import { useEffect, useState } from "react";
import { t, type Page } from "@decka/shared";
import { call } from "../ipc";
import Button from "../ui/Button";
import { normalizeApp, withApps } from "./layoutOps";

interface Props {
  page: Page;
  /** The first page is the home page and cannot be a context page. */
  isFirst: boolean;
  onChange: (page: Page) => void;
}

/** "Show this page automatically when active": the applications that open this page on the phone. */
export default function PageApps({ page, isFirst, onChange }: Props) {
  const [text, setText] = useState("");
  const [suggestions, setSuggestions] = useState<string[]>([]);
  const apps = page.apps ?? [];

  const refresh = () => {
    call("list_window_apps").then(setSuggestions, () => setSuggestions([]));
  };
  useEffect(refresh, []);

  function add() {
    const name = normalizeApp(text);
    if (name === null) return;
    setText("");
    if (!apps.includes(name)) onChange(withApps(page, [...apps, name]));
  }

  if (isFirst) {
    return (
      <section className="card">
        <h3>{t("pageapps.title")}</h3>
        <p className="muted">{t("pageapps.home")}</p>
      </section>
    );
  }

  const listId = `apps-${page.id}`;
  return (
    <section className="card">
      <h3>{t("pageapps.title")}</h3>
      <div className="row wrap">
        {apps.map((app) => (
          <span key={app} className="chip">
            {app}
            <button type="button" className="chip-x" aria-label={t("pageapps.remove", { app })} onClick={() => onChange(withApps(page, apps.filter((a) => a !== app)))}>
              ×
            </button>
          </span>
        ))}
      </div>
      <div className="row">
        <input
          aria-label={t("pageapps.application")}
          list={listId}
          placeholder="code.exe"
          value={text}
          onChange={(e) => setText(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === "Enter") {
              e.preventDefault();
              add();
            }
          }}
        />
        <datalist id={listId}>
          {suggestions.filter((s) => !apps.includes(s)).map((s) => <option key={s} value={s} />)}
        </datalist>
        <Button onClick={add}>{t("picker.add")}</Button>
        <Button onClick={refresh}>{t("pageapps.refresh")}</Button>
      </div>
      <p className="muted">{t("pageapps.hint")}</p>
    </section>
  );
}
