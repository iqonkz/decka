import { useEffect, useMemo, useState } from "react";
import type { ActionsFile, InstalledApp } from "@decka/shared";
import { call } from "../ipc";
import type { DragPayload } from "./dragMachine";
import type { Kind } from "./layoutOps";

type Tab = "apps" | "actions" | "widgets";

const WIDGETS: { kind: Kind; name: string }[] = [
  { kind: "clock", name: "Clock" },
  { kind: "sys", name: "System" },
  { kind: "media", name: "Now playing" },
  { kind: "statuses", name: "Statuses" },
  { kind: "status", name: "Status tile" },
  { kind: "notifications", name: "Notifications" },
];

/** The palette is loaded once per window session, like the app picker. */
let appsCache: InstalledApp[] | null = null;

/** Things to drag onto the canvas: programs, actions, widgets (DESIGN.md 4.4). */
export default function Palette({ actions, onBegin }: { actions: ActionsFile; onBegin: (payload: DragPayload, e: React.PointerEvent) => void }) {
  const [tab, setTab] = useState<Tab>("apps");
  const [query, setQuery] = useState("");
  const [apps, setApps] = useState<InstalledApp[] | null>(appsCache);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (tab !== "apps" || appsCache !== null) return;
    call("list_installed_apps").then(
      (list) => {
        appsCache = list;
        setApps(list);
      },
      (e) => setError(String(e)),
    );
  }, [tab]);

  const q = query.trim().toLowerCase();
  const shownApps = useMemo(() => (apps ?? []).filter((a) => q === "" || a.name.toLowerCase().includes(q)), [apps, q]);
  const shownActions = useMemo(
    () => Object.entries(actions.actions).filter(([id, a]) => q === "" || a.name.toLowerCase().includes(q) || id.includes(q)).sort(([, a], [, b]) => a.name.localeCompare(b.name)),
    [actions, q],
  );

  return (
    <aside className="palette">
      <div className="row wrap" role="tablist">
        {(["apps", "actions", "widgets"] as Tab[]).map((t) => (
          <button key={t} type="button" role="tab" aria-selected={tab === t} className={tab === t ? "page-tab active" : "page-tab"} onClick={() => setTab(t)}>
            {t === "apps" ? "Apps" : t === "actions" ? "Actions" : "Widgets"}
          </button>
        ))}
      </div>
      {tab !== "widgets" && <input aria-label="Search" placeholder="Search" value={query} onChange={(e) => setQuery(e.target.value)} />}
      <p className="muted palette-hint">Drag onto a cell of the preview.</p>
      <div className="palette-list">
        {tab === "apps" && apps === null && !error && <p className="muted">Loading installed apps…</p>}
        {tab === "apps" && error && <p className="warn-text">{error}</p>}
        {tab === "apps" &&
          shownApps.map((app) => (
            <div key={`${app.program}|${app.args.join(" ")}|${app.name}`} className="palette-item" onPointerDown={(e) => onBegin({ type: "app", app }, e)}>
              {app.icon ? <img src={app.icon} width={24} height={24} alt="" /> : <span className="app-noicon" style={{ width: 24, height: 24 }} />}
              <span>{app.name}</span>
            </div>
          ))}
        {tab === "actions" &&
          shownActions.map(([id, a]) => (
            <div key={id} className="palette-item" onPointerDown={(e) => onBegin({ type: "action", actionId: id, label: a.name }, e)}>
              <span>{a.name}</span>
              <small className="muted">{id}</small>
            </div>
          ))}
        {tab === "widgets" &&
          WIDGETS.map((w) => (
            <div key={w.kind} className="palette-item" onPointerDown={(e) => onBegin({ type: "widget", kind: w.kind }, e)}>
              <span>{w.name}</span>
            </div>
          ))}
      </div>
    </aside>
  );
}
