import { useEffect, useMemo, useState } from "react";
import type { InstalledApp } from "@decka/shared";
import { call } from "../ipc";
import Banner from "../ui/Banner";
import Button from "../ui/Button";
import { filterApps } from "./programPick";

/** The list is read once per settings-window session (it takes seconds); `Refresh` reads it again. */
let cache: InstalledApp[] | null = null;

interface Props {
  onPick: (app: InstalledApp) => void;
  onClose: () => void;
}

/** Search and choose one of the programs from the Start menu. */
export default function AppPickerDialog({ onPick, onClose }: Props) {
  const [apps, setApps] = useState<InstalledApp[] | null>(cache);
  const [query, setQuery] = useState("");
  const [error, setError] = useState<string | null>(null);

  function load() {
    setApps(null);
    setError(null);
    call("list_installed_apps").then(
      (list) => {
        cache = list;
        setApps(list);
      },
      (e) => setError(String(e)),
    );
  }

  useEffect(() => {
    if (cache === null) load();
  }, []);

  const shown = useMemo(() => (apps ? filterApps(apps, query) : []), [apps, query]);

  return (
    <div className="dialog-backdrop">
      <div className="dialog" role="dialog" aria-label="Choose installed app">
        <h2>Choose installed app</h2>
        <div className="row">
          <input autoFocus placeholder="Search" aria-label="Search" value={query} onChange={(e) => setQuery(e.target.value)} />
          <Button onClick={load}>Refresh</Button>
        </div>
        {error && <Banner kind="error">{error}</Banner>}
        {apps === null && !error && <p className="muted">Loading installed apps…</p>}
        {apps !== null && shown.length === 0 && <p className="muted">No programs found.</p>}
        <div className="app-list">
          {shown.map((app) => (
            <button key={`${app.program}|${app.args.join(" ")}|${app.name}`} type="button" className="app-row" onClick={() => onPick(app)}>
              {app.icon ? <img src={app.icon} width={32} height={32} alt="" /> : <span className="app-noicon" />}
              <span className="app-text">
                <strong>{app.name}</strong>
                <small className="muted">{app.program}</small>
              </span>
            </button>
          ))}
        </div>
        <div className="row">
          <Button onClick={onClose}>Close</Button>
        </div>
      </div>
    </div>
  );
}
