import { useEffect, useMemo, useState } from "react";
import EntryIcon from "../ui/EntryIcon";
import { CATALOG, SETS, catalogItem, categoryItems, getLanguage, insertSet, searchCatalog, t, type ActionsFile, type Action, type CatalogItem, type CatalogSet, type InstalledApp, type LayoutFile } from "@decka/shared";
import PhIcon from "../ui/PhIcon";
import { describeAction } from "../editor/describeAction";
import { cachedApps, loadApps } from "../editor/apps";
import { libraryCategories, type Integrations } from "../editor/catalogUi";
import type { DragPayload } from "./dragMachine";

type Tab = "library" | "sets" | "apps";
const OPEN_KEY = "decka.library.open.v1";

function readOpen(): string[] {
  try {
    const raw = JSON.parse(localStorage.getItem(OPEN_KEY) ?? "null") as unknown;
    if (Array.isArray(raw)) return raw.filter((x): x is string => typeof x === "string");
  } catch {
    // no storage: the first category is open
  }
  return [libraryCategories()[0]?.id ?? ""];
}

/** What an item does, in grey on the right: the keys of a shortcut, the size of a widget, `OBS`. */
function hintOf(item: CatalogItem): string {
  if (item.widget) return `${item.widget.w}×${item.widget.h}`;
  if (item.action) {
    const text = describeAction({ name: item.name[getLanguage()], ...item.action } as Action);
    return text === item.name[getLanguage()] ? (item.requires === "obs" ? "OBS" : "") : text;
  }
  return item.requires === "obs" ? "OBS" : "";
}

export interface LibraryProps {
  layout: LayoutFile;
  actions: ActionsFile;
  pageIndex: number;
  integrations: Integrations;
  onBegin: (payload: DragPayload, e: React.PointerEvent) => void;
  onAddItem: (itemId: string) => void;
  onAddSet: (set: CatalogSet) => void;
  onSetup: (what: "obs" | "weather") => void;
}

/** The left panel of the editor (DESIGN.md 4.8): the library of ready-made keys, the sets, and the installed programs. */
export default function Library({ layout, actions, pageIndex, integrations, onBegin, onAddItem, onAddSet, onSetup }: LibraryProps) {
  const [tab, setTab] = useState<Tab>("library");
  const [query, setQuery] = useState("");
  const [open, setOpen] = useState<string[]>(readOpen);
  const [apps, setApps] = useState<InstalledApp[] | null>(cachedApps());
  const lang = getLanguage();

  useEffect(() => {
    if (tab === "apps" && apps === null) loadApps().then(setApps, () => setApps([]));
  }, [tab, apps]);
  useEffect(() => {
    try {
      localStorage.setItem(OPEN_KEY, JSON.stringify(open));
    } catch {
      // not remembered
    }
  }, [open]);

  const toggle = (id: string) => setOpen((list) => (list.includes(id) ? list.filter((x) => x !== id) : [...list, id]));
  const q = query.trim().toLowerCase();
  const found = useMemo(() => (q === "" ? null : searchCatalog(q)), [q]);
  const ready = (item: { requires?: "obs" | "weather" }) => item.requires === undefined || integrations[item.requires];
  const page = layout.pages[pageIndex];

  const itemRow = (item: CatalogItem) => {
    const waiting = !ready(item);
    return (
      <div
        key={item.id}
        className={waiting ? "palette-item library-item waiting" : "palette-item library-item"}
        data-item={item.id}
        onPointerDown={(e) => !waiting && onBegin({ type: "catalog", itemId: item.id, label: item.name[lang] }, e)}
        onDoubleClick={() => (waiting ? onSetup(item.requires!) : onAddItem(item.id))}
        title={waiting ? t(item.requires === "obs" ? "library.setup_obs" : "library.setup_weather") : t("library.hint")}
      >
        <EntryIcon icon={item.icon} />
        <span>{item.name[lang]}</span>
        <small className="muted">{waiting ? t("library.setup") : hintOf(item)}</small>
      </div>
    );
  };

  // Where a set would go on this page (a new page, when it does not fit): looked up without any side effect.
  const newPageFor = (set: CatalogSet): string | null => {
    if (!page) return null;
    let n = 0; // a spread of different "random" numbers: the ids of the keys must differ
    const r = insertSet(layout, actions, page.id, set, { language: lang, scenes: ["a", "b", "c"], inputs: ["a"], random: () => (n++ * 0.6180339887) % 1 });
    return r.ok && r.placedOn[0] !== undefined && r.placedOn[0] !== page.id ? set.name[lang] : null;
  };

  return (
    <aside className="palette library">
      <input aria-label={t("library.search")} placeholder={t("library.search")} value={query} onChange={(e) => setQuery(e.target.value)} />
      <div className="row wrap" role="tablist">
        {(["library", "sets", "apps"] as Tab[]).map((id) => (
          <button key={id} type="button" role="tab" aria-selected={tab === id} className={tab === id ? "page-tab active" : "page-tab"} onClick={() => setTab(id)}>
            {t(`library.tab_${id}`)}
          </button>
        ))}
      </div>
      <div className="palette-list">
        {tab === "library" && found !== null && (found.length === 0 ? <p className="muted">{t("library.nothing")}</p> : found.map(itemRow))}
        {tab === "library" &&
          found === null &&
          libraryCategories().map((category) => {
            const expanded = open.includes(category.id);
            return (
              <div key={category.id} className="library-category">
                <button type="button" className="library-head" aria-expanded={expanded} onClick={() => toggle(category.id)}>
                  <PhIcon name={expanded ? "caret-down" : "caret-right"} size={14} />
                  <EntryIcon icon={category.icon} size={16} />
                  <span>{category.name[lang]}</span>
                </button>
                {expanded && categoryItems(category).map(itemRow)}
              </div>
            );
          })}

        {tab === "sets" &&
          SETS.filter((s) => q === "" || s.name.en.toLowerCase().includes(q) || s.name.ru.toLowerCase().includes(q) || s.description[lang].toLowerCase().includes(q)).map((set) => {
            const waiting = !ready(set);
            const goesTo = waiting ? null : newPageFor(set);
            return (
              <div key={set.id} className="set-card" data-set={set.id} onPointerDown={(e) => !waiting && onBegin({ type: "set", setId: set.id, label: set.name[lang] }, e)}>
                <div className="set-head">
                  <EntryIcon icon={set.icon} />
                  <strong>{set.name[lang]}</strong>
                </div>
                <p className="muted set-text">{set.description[lang]}</p>
                <div className="set-foot">
                  <SetPreview set={set} />
                  <button type="button" className={waiting ? "set-add waiting" : "set-add"} onPointerDown={(e) => e.stopPropagation()} onClick={() => (waiting ? onSetup(set.requires!) : onAddSet(set))}>
                    {waiting ? t(set.requires === "obs" ? "library.setup_obs" : "library.setup_weather") : t("library.add")}
                  </button>
                </div>
                {goesTo && <small className="muted">{t("library.new_page", { name: goesTo })}</small>}
              </div>
            );
          })}

        {tab === "apps" && apps === null && <p className="muted">…</p>}
        {tab === "apps" &&
          (apps ?? [])
            .filter((a) => q === "" || a.name.toLowerCase().includes(q))
            .map((app) => (
              <div key={`${app.program}|${app.args.join(" ")}|${app.name}`} className="palette-item" onPointerDown={(e) => onBegin({ type: "app", app }, e)}>
                {app.icon ? <img src={app.icon} width={24} height={24} alt="" /> : <span className="app-noicon" style={{ width: 24, height: 24 }} />}
                <span>{app.name}</span>
              </div>
            ))}
      </div>
      <p className="muted palette-hint">{t("library.hint")}</p>
    </aside>
  );
}

/** The form of a set drawn with 14 px keys. */
function SetPreview({ set }: { set: CatalogSet }) {
  const cell = 14;
  const gap = 2;
  const cols = Math.max(...set.rows.map((r) => r.length));
  const boxes: { id: string; x: number; y: number; w: number; h: number }[] = [];
  set.rows.forEach((row, y) => row.forEach((id, x) => id !== null && boxes.push({ id, x, y, w: catalogItem(id, CATALOG)?.widget?.w ?? 1, h: catalogItem(id, CATALOG)?.widget?.h ?? 1 })));
  return (
    <svg className="set-preview" width={cols * (cell + gap)} height={set.rows.length * (cell + gap)} aria-hidden="true">
      {boxes.map((b, i) => (
        <rect key={i} x={b.x * (cell + gap)} y={b.y * (cell + gap)} width={b.w * cell + (b.w - 1) * gap} height={b.h * cell + (b.h - 1) * gap} rx={3} fill={b.w > 1 || b.h > 1 ? "var(--accent, #7c5cff)" : "currentColor"} opacity={b.w > 1 || b.h > 1 ? 0.7 : 0.45} />
      ))}
    </svg>
  );
}
