import { useEffect, useMemo, useRef, useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import EntryIcon from "../ui/EntryIcon";
import { CATALOG, categoryItems, getLanguage, pickText, t, type ActionsFile, type InstalledApp, type Page } from "@decka/shared";
import { call } from "../ipc";
import Button from "../ui/Button";
import PhIcon from "../ui/PhIcon";
import { DEFAULT_SIZE, fits, type Grid, type Kind } from "../tabs/layoutOps";
import { actionFromKeys, actionFromUrl, defaultIconFor, formatChord, type Choice } from "./addButton";
import { cachedApps, loadApps } from "./apps";
import { chordFromEvent } from "./keyName";

const widgets = (): { kind: Kind; name: string }[] => [
  { kind: "clock", name: t("widget_name.clock") },
  { kind: "sys", name: t("widget_name.sys") },
  { kind: "media", name: t("widget_name.media") },
  { kind: "statuses", name: t("widget_name.statuses") },
  { kind: "status", name: t("widget_name.status") },
  { kind: "notifications", name: t("widget_name.notifications") },
];

interface Row {
  key: string;
  group: string;
  label: string;
  icon?: string;
  phIcon?: string;
  hint?: string;
  /** The header of the group: the name of a catalog category, `Apps` or `Your actions`. */
  groupLabel: string;
  choice: Choice;
  /** A library item: the host places it (it may ask for more first). */
  catalogId?: string;
}

interface Props {
  /** The cell's rectangle on screen: the popup opens next to it. */
  anchor: { left: number; top: number; bottom: number };
  cell: { x: number; y: number };
  page: Page;
  grid: Grid;
  actions: ActionsFile;
  /** Changing the action of a key: no widgets in the list. */
  hideWidgets?: boolean;
  /** Only the `Apps` group and `Browse for a program…` (the action form picks a program). */
  appsOnly?: boolean;
  onChoose: (choice: Choice) => void;
  /** A library item was picked (DESIGN.md 4.8: the same categories as the library). */
  onCatalog?: (itemId: string) => void;
  onClose: () => void;
}

type Mode = "list" | "website" | "keys";

/** The quick picker (DESIGN.md 4.5): one popup next to an empty cell; one click on a row makes the button. */
export default function QuickPicker({ anchor, cell, page, grid, actions, hideWidgets, appsOnly, onChoose, onCatalog, onClose }: Props) {
  const lang = getLanguage();
  const pick = (row: Row) => (row.catalogId !== undefined && onCatalog ? onCatalog(row.catalogId) : onChoose(row.choice));
  const [query, setQuery] = useState("");
  const [apps, setApps] = useState<InstalledApp[] | null>(cachedApps());
  const [active, setActive] = useState(0);
  const [mode, setMode] = useState<Mode>("list");
  const [url, setUrl] = useState("");
  const [chord, setChord] = useState<string[]>([]);
  const [hint, setHint] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const listRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (apps === null) loadApps().then(setApps, (e) => setError(String(e)));
  }, [apps]);

  const rows = useMemo<Row[]>(() => {
    const q = query.trim().toLowerCase();
    const match = (s: string) => q === "" || s.toLowerCase().includes(q);
    const out: Row[] = [];
    for (const app of apps ?? []) {
      if (match(app.name)) out.push({ key: `app|${app.program}|${app.args.join(" ")}|${app.name}`, group: "apps", groupLabel: t("picker.group_apps"), label: app.name, icon: app.icon, hint: undefined, choice: { type: "app", app } });
    }
    // The categories of the library, in its order (DESIGN.md 4.8); a widget only when it fits the cell, and not when a key's action is being changed.
    for (const category of appsOnly || !onCatalog ? [] : CATALOG.categories) {
      for (const item of categoryItems(category)) {
        if (item.widget && (hideWidgets || !fits(page, grid, { x: cell.x, y: cell.y, w: item.widget.w, h: item.widget.h }))) continue;
        if (!(match(pickText(item.name, lang)) || match(item.name.en) || match(item.name.ru ?? "") || match(pickText(item.label, lang)))) continue;
        out.push({ key: `catalog|${item.id}`, group: category.id, groupLabel: pickText(category.name, lang), label: pickText(item.name, lang), phIcon: item.icon, hint: item.widget ? `${item.widget.w}×${item.widget.h}` : undefined, choice: { type: "action", actionId: "" }, catalogId: item.id });
      }
    }
    const yours = Object.entries(appsOnly ? {} : actions.actions)
      .filter(([id, a]) => match(a.name) || match(id))
      .sort(([, a], [, b]) => a.name.localeCompare(b.name));
    for (const [id, a] of yours) out.push({ key: `action|${id}`, group: "yours", groupLabel: t("picker.group_yours"), label: a.name, hint: defaultIconFor(a).replace("lucide:", ""), choice: { type: "action", actionId: id } });
    // Widgets that the catalog does not have (the status tile) stay, as before.
    for (const w of hideWidgets || appsOnly ? [] : widgets().filter((x) => x.kind === "status")) {
      const size = DEFAULT_SIZE[w.kind];
      if (match(w.name) && fits(page, grid, { x: cell.x, y: cell.y, w: size.w, h: size.h })) out.push({ key: `widget|${w.kind}`, group: "widgets", groupLabel: t("picker.group_widgets"), label: w.name, hint: `${size.w}×${size.h}`, choice: { type: "widget", kind: w.kind } });
    }
    return out;
  }, [apps, actions, query, page, grid, cell.x, cell.y, hideWidgets, appsOnly, onCatalog, lang]);

  useEffect(() => setActive(0), [query]);
  useEffect(() => {
    listRef.current?.querySelector<HTMLElement>('[data-active="true"]')?.scrollIntoView({ block: "nearest" });
  }, [active, rows]);

  // Keyboard: arrows and Enter in the list, Esc anywhere. Shortcut recording takes every key (Esc cancels it).
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (mode === "keys") {
        e.preventDefault();
        e.stopPropagation();
        if (e.key === "Escape") return setMode("list");
        if (e.key === "Enter") {
          if (chord.length > 0 && actionFromKeys(chord)) onChoose({ type: "keys", keys: chord });
          return;
        }
        const next = chordFromEvent(e);
        setHint(next.unsupported ? t("picker.key_unsupported") : null);
        if (next.keys.length > 0) setChord(next.keys);
        return;
      }
      if (e.key === "Escape") {
        e.preventDefault();
        if (mode === "website") setMode("list");
        else onClose();
      } else if (mode === "list" && e.key === "ArrowDown") {
        e.preventDefault();
        setActive((i) => Math.min(rows.length - 1, i + 1));
      } else if (mode === "list" && e.key === "ArrowUp") {
        e.preventDefault();
        setActive((i) => Math.max(0, i - 1));
      } else if (mode === "list" && e.key === "Enter") {
        const row = rows[active];
        if (row) {
          e.preventDefault();
          pick(row);
        }
      }
    };
    window.addEventListener("keydown", onKey, true);
    return () => window.removeEventListener("keydown", onKey, true);
  }, [mode, chord, rows, active, onChoose, onCatalog, onClose]);

  async function browse() {
    try {
      const picked = await open({ multiple: false, directory: false, filters: [{ name: t("picker.programs"), extensions: ["exe", "lnk"] }] });
      if (typeof picked !== "string") return;
      const app = await call("describe_program", { path: picked });
      if ("error" in app) return setError(app.error);
      onChoose({ type: "app", app });
    } catch (e) {
      setError(String(e));
    }
  }

  function confirmUrl() {
    if (!actionFromUrl(url)) return setError(t("picker.enter_url"));
    onChoose({ type: "url", url });
  }

  const width = 360;
  const left = Math.max(8, Math.min(anchor.left, window.innerWidth - width - 8));
  const top = Math.max(8, Math.min(anchor.bottom + 6, window.innerHeight - 440));

  let lastGroup = "";
  return (
    <>
      <div className="picker-backdrop" onPointerDown={onClose} />
      <div className="quick-picker" role="dialog" aria-label={t("picker.add_button")} style={{ left, top, width }}>
        {mode === "list" && (
          <>
            <input autoFocus aria-label={t("picker.search")} placeholder={t("picker.search")} value={query} onChange={(e) => setQuery(e.target.value)} />
            <div className="picker-list" ref={listRef}>
              {apps === null && !error && <p className="muted picker-note">{t("picker.loading_apps")}</p>}
              {rows.length === 0 && apps !== null && <p className="muted picker-note">{t("picker.nothing_found")}</p>}
              {rows.map((row, i) => {
                const header = row.group !== lastGroup ? row.group : null;
                lastGroup = row.group;
                return (
                  <div key={row.key}>
                    {header && <div className="picker-group">{row.groupLabel}</div>}
                    <button type="button" className={`picker-row${i === active ? " active" : ""}`} data-active={i === active} onMouseEnter={() => setActive(i)} onClick={() => pick(row)}>
                      {row.icon ? <img src={row.icon} width={24} height={24} alt="" /> : row.phIcon ? <span className="picker-icon"><EntryIcon icon={row.phIcon} /></span> : <span className="app-noicon" style={{ width: 24, height: 24 }} />}
                      <span className="picker-label">{row.label}</span>
                      {row.hint && <small className="muted">{row.hint}</small>}
                    </button>
                  </div>
                );
              })}
            </div>
            <div className="picker-bottom">
              <button type="button" className="picker-row" onClick={() => void browse()}>
                <PhIcon name="folder-open" size={18} />
                <span className="picker-label">{t("picker.browse")}</span>
              </button>
              {!appsOnly && <button type="button" className="picker-row" onClick={() => { setMode("website"); setError(null); }}>
                <PhIcon name="globe" size={18} />
                <span className="picker-label">{t("picker.website")}</span>
              </button>}
              {!appsOnly && <button type="button" className="picker-row" onClick={() => { setMode("keys"); setChord([]); setHint(null); setError(null); }}>
                <PhIcon name="keyboard" size={18} />
                <span className="picker-label">{t("picker.shortcut")}</span>
              </button>}
            </div>
          </>
        )}
        {mode === "website" && (
          <div className="picker-form">
            <strong>{t("form.type_website")}</strong>
            <input
              autoFocus
              aria-label={t("picker.web_address")}
              placeholder="github.com"
              value={url}
              onChange={(e) => { setUrl(e.target.value); setError(null); }}
              onKeyDown={(e) => { if (e.key === "Enter") { e.preventDefault(); e.stopPropagation(); confirmUrl(); } }}
            />
            <div className="row">
              <Button variant="primary" onClick={confirmUrl}>{t("picker.add")}</Button>
              <Button onClick={() => setMode("list")}>{t("picker.back")}</Button>
            </div>
          </div>
        )}
        {mode === "keys" && (
          <div className="picker-form">
            <strong>{t("picker.shortcut_title")}</strong>
            <div className="chord" aria-live="polite">{chord.length > 0 ? formatChord(chord) : t("picker.press_keys")}</div>
            {hint && <small className="warn-text">{hint}</small>}
            <small className="muted">{t("picker.shortcut_help")}</small>
          </div>
        )}
        {error && <p className="warn-text picker-note">{error}</p>}
      </div>
    </>
  );
}