import { useEffect, useState } from "react";
import type { Page } from "@decka/shared";
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
        <h3>Show this page automatically when active</h3>
        <p className="muted">The first page is your home page.</p>
      </section>
    );
  }

  const listId = `apps-${page.id}`;
  return (
    <section className="card">
      <h3>Show this page automatically when active</h3>
      <div className="row wrap">
        {apps.map((app) => (
          <span key={app} className="chip">
            {app}
            <button type="button" className="chip-x" aria-label={`Remove ${app}`} onClick={() => onChange(withApps(page, apps.filter((a) => a !== app)))}>
              ×
            </button>
          </span>
        ))}
      </div>
      <div className="row">
        <input
          aria-label="Application"
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
        <Button onClick={add}>Add</Button>
        <Button onClick={refresh}>Refresh</Button>
      </div>
      <p className="muted">The phone switches to this page while one of these apps is in front on your PC, and switches back when you leave it.</p>
    </section>
  );
}
