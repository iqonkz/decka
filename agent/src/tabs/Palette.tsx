import { useEffect, useMemo, useState } from "react";
import { t, type ActionsFile, type InstalledApp } from "@decka/shared";
import { call } from "../ipc";
import type { DragPayload } from "./dragMachine";
import type { Kind } from "./layoutOps";

type Tab = "apps" | "actions" | "widgets";

const widgets = (): { kind: Kind; name: string }[] => [
  { kind: "clock", name: t("widget_name.clock") },
  { kind: "sys", name: t("widget_name.sys") },
  { kind: "media", name: t("widget_name.media") },
  { kind: "statuses", name: t("widget_name.statuses") },
  { kind: "status", name: t("widget_name.status") },
  { kind: "notifications", name: t("widget_name.notifications") },
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
        {(["apps", "actions", "widgets"] as Tab[]).map((id) => (
          <button key={id} type="button" role="tab" aria-selected={tab === id} className={tab === id ? "page-tab active" : "page-tab"} onClick={() => setTab(id)}>
            {t(`palette.${id}`)}
          </button>
        ))}
      </div>
      {tab !== "widgets" && <input aria-label={t("wizard.search")} placeholder={t("wizard.search")} value={query} onChange={(e) => setQuery(e.target.value)} />}
      <p className="muted palette-hint">{t("palette.hint")}</p>
      <div className="palette-list">
        {tab === "apps" && apps === null && !error && <p className="muted">{t("picker.loading_apps")}</p>}
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
          widgets().map((w) => (
            <div key={w.kind} className="palette-item" onPointerDown={(e) => onBegin({ type: "widget", kind: w.kind }, e)}>
              <span>{w.name}</span>
            </div>
          ))}
      </div>
    </aside>
  );
}
