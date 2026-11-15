import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import type { ActionsFile, DisplaySettings, LayoutFile, LayoutItem, ValidationIssue } from "@decka/shared";
import { validateLayout } from "@decka/shared";
import { call } from "../ipc";
import Banner from "../ui/Banner";
import Button from "../ui/Button";
import Field from "../ui/Field";
import { canRedo, canUndo, itemAt, newHistory, pushHistory, redo, undo, type History } from "./editorOps";
import EditorCanvas, { type DragPayload } from "./EditorCanvas";
import Palette from "./Palette";
import ItemForm from "./itemForm";
import { describeIssue, itemPath } from "./issueText";
import { addItem, COLS_RANGE, DEFAULT_SIZE, makeItem, movePage, newPageId, newPopupPage, removeItem, ROWS_RANGE, updateItem, withMediaDefaults, type Kind } from "./layoutOps";
import PageApps from "./PageApps";
import TemplatesDialog from "./TemplatesDialog";

const KINDS: Kind[] = ["button", "clock", "sys", "status", "statuses", "notifications", "media"];

/** A new button starts with the first action by name; without actions it opens another page. */
function withButtonDefault(item: LayoutItem, actions: ActionsFile, pages: LayoutFile["pages"], currentPageId: string): LayoutItem {
  if (item.kind !== "button") return item;
  const first = Object.entries(actions.actions).sort(([, a], [, b]) => a.name.localeCompare(b.name))[0];
  if (first) return { ...item, actionId: first[0] };
  const other = pages.find((p) => p.id !== currentPageId);
  return other ? { ...item, navigate: other.id } : item;
}

const sameLayout = (a: LayoutFile, b: LayoutFile) => JSON.stringify(a) === JSON.stringify(b);

/** `Notepad` -> `app-notepad`; the first free id when it is taken. */
function newAppActionId(name: string, taken: Record<string, unknown>): string {
  const base = `app-${name.toLowerCase().replace(/[^a-z0-9]+/g, "-").replace(/^-+|-+$/g, "").slice(0, 50) || "program"}`;
  if (!(base in taken)) return base;
  for (let n = 2; ; n++) if (!(`${base}-${n}` in taken)) return `${base}-${n}`;
}

export default function LayoutTab() {
  // The layout being edited, with 50 steps of undo/redo; `saved` is what is on the PC now.
  const [hist, setHist] = useState<History<LayoutFile> | null>(null);
  const [saved, setSaved] = useState("");
  const layout = hist?.present ?? null;
  const [actions, setActions] = useState<ActionsFile | null>(null);
  const [mainWidth, setMainWidth] = useState(640);
  const mainRef = useRef<HTMLDivElement>(null);
  const saveRef = useRef<() => void>(() => {});
  const [pageIdx, setPageIdx] = useState(0);
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [creating, setCreating] = useState<{ x: number; y: number } | null>(null);
  const [renaming, setRenaming] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const [serverIssues, setServerIssues] = useState<ValidationIssue[]>([]);
  const [message, setMessage] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [templatesOpen, setTemplatesOpen] = useState(false);

  const load = useCallback(async () => {
    const fresh = await call("get_layout");
    setHist(newHistory(fresh));
    setSaved(JSON.stringify(fresh));
    setActions(await call("get_actions"));
    setServerIssues([]);
    setMessage(null);
    setSelectedId(null);
    setCreating(null);
    return fresh;
  }, []);

  useEffect(() => {
    load().catch((e) => setError(String(e)));
  }, [load]);

  // The preview takes the width of its column.
  const loaded = layout !== null;
  useEffect(() => {
    const el = mainRef.current;
    if (!el) return;
    const measure = () => setMainWidth(el.clientWidth);
    measure();
    const observer = new ResizeObserver(measure);
    observer.observe(el);
    return () => observer.disconnect();
  }, [loaded]);

  // Ctrl+Z / Ctrl+Y / Ctrl+S (text fields keep their own undo).
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (!(e.ctrlKey || e.metaKey)) return;
      const key = e.key.toLowerCase();
      const typing = e.target instanceof HTMLElement && /^(INPUT|TEXTAREA|SELECT)$/.test(e.target.tagName);
      if (key === "s") {
        e.preventDefault();
        saveRef.current();
      } else if (!typing && key === "z") {
        e.preventDefault();
        setHist((h) => (h ? (e.shiftKey ? redo(h) : undo(h)) : h));
      } else if (!typing && key === "y") {
        e.preventDefault();
        setHist((h) => (h ? redo(h) : h));
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, []);

  const issues = useMemo(() => (layout && actions ? validateLayout(layout, actions) : []), [layout, actions]);
  const shownIssues = [...issues, ...serverIssues.filter((s) => !issues.some((i) => i.path === s.path && i.message === s.message))];

  if (error) return <Banner kind="error">{error}</Banner>;
  if (!layout || !actions) return <p>Loading…</p>;

  const page = layout.pages[pageIdx] ?? layout.pages[0];
  if (!page) return <Banner kind="error">The layout has no pages.</Banner>;
  const selected = page.items.find((i) => i.id === selectedId) ?? null;
  const badItems = new Set(
    shownIssues.map((i) => itemPath(i.path)).filter((p): p is [number, number] => p !== null && p[0] === pageIdx).map(([, n]) => page.items[n]?.id),
  );

  const edit = (next: LayoutFile) => {
    setHist((h) => (h ? pushHistory(h, next, sameLayout) : h));
    setServerIssues([]);
    setMessage(null);
  };
  const dirty = JSON.stringify(layout) !== saved;
  const step = (change: (h: History<LayoutFile>) => History<LayoutFile>) => {
    setHist((h) => (h ? change(h) : h));
    setServerIssues([]);
    setMessage(null);
  };
  const setPage = (p: typeof page) => edit({ ...layout, pages: layout.pages.map((x, i) => (i === pageIdx ? p : x)) });
  const setDisplay = (d: Partial<DisplaySettings>) => edit({ ...layout, display: { ...layout.display, ...d } });
  // Items are never moved by a size change: those that no longer fit are flagged by validation (red) and block Save.
  const setGrid = (g: Partial<LayoutFile["grid"]>) => edit({ ...layout, grid: { ...layout.grid, ...g } });
  const { cols, rows } = layout.grid;
  const options = (range: readonly [number, number]) => Array.from({ length: range[1] - range[0] + 1 }, (_, i) => range[0] + i);

  function create(kind: Kind) {
    if (!creating || !page) return;
    const fresh = withButtonDefault(withMediaDefaults(makeItem(kind, creating.x, creating.y), Object.keys(actions!.actions)), actions!, layout!.pages, page.id);
    const result = addItem(page, layout!.grid, fresh);
    if (!result.ok) return setNotice(result.error);
    const added = result.page.items[result.page.items.length - 1];
    setNotice(null);
    setPage(result.page);
    setCreating(null);
    setSelectedId(added?.id ?? null);
  }

  /** Clicking an issue opens the page and the form of the element it is about. */
  function selectIssue(path: string) {
    const p = itemPath(path);
    const target = p ? layout!.pages[p[0]]?.items[p[1]] : undefined;
    if (!p || !target) return;
    setPageIdx(p[0]);
    setSelectedId(target.id);
    setCreating(null);
  }

  function addPage() {
    const n = layout!.pages.length + 1;
    edit({ ...layout!, pages: [...layout!.pages, { id: newPageId(), name: `Page ${n}`, items: [] }] });
    setPageIdx(layout!.pages.length);
    setSelectedId(null);
  }

  function addPopupPage() {
    edit({ ...layout!, pages: [...layout!.pages, newPopupPage(layout!.pages)] });
    setPageIdx(layout!.pages.length);
    setSelectedId(null);
  }

  function deletePage() {
    if (layout!.pages.length <= 1) return;
    edit({ ...layout!, pages: layout!.pages.filter((_, i) => i !== pageIdx) });
    setPageIdx(Math.max(0, pageIdx - 1));
    setSelectedId(null);
  }

  function shiftPage(delta: number) {
    edit(movePage(layout!, pageIdx, delta));
    setPageIdx(Math.min(layout!.pages.length - 1, Math.max(0, pageIdx + delta)));
  }

  async function save() {
    if (shownIssues.length > 0) return;
    const result = await call("save_layout", { layout: layout! });
    if (result.ok) {
      setServerIssues([]);
      setMessage("Saved");
      setSaved(JSON.stringify(layout));
    } else {
      setServerIssues(result.issues);
      setMessage(null);
    }
  }
  saveRef.current = () => void save();

  /** Something from the palette was dropped on cell (x, y): a program, an action or a widget becomes an item. */
  async function dropOnCell(payload: DragPayload, x: number, y: number) {
    setNotice(null);
    const ids = Object.keys(actions!.actions);
    let item: LayoutItem | null = null;
    if (payload.type === "widget") {
      item = withMediaDefaults(makeItem(payload.kind, x, y), ids);
    } else if (payload.type === "action") {
      item = { ...makeItem("button", x, y), label: payload.label.slice(0, 24), actionId: payload.actionId } as LayoutItem;
    } else if (payload.type === "app") {
      const { app } = payload;
      if (itemAt(page!, x, y)) return setNotice("Drop a program on an empty cell.");
      const actionId = newAppActionId(app.name, actions!.actions);
      const next: ActionsFile = {
        ...actions!,
        actions: { ...actions!.actions, [actionId]: { name: app.name.slice(0, 60), type: "launch", program: app.program, ...(app.args.length > 0 ? { args: app.args } : {}), ...(app.cwd ? { cwd: app.cwd } : {}) } },
      };
      const result = await call("save_actions", { actions: next });
      if (!result.ok) return setNotice(result.issues[0]?.message ?? "Cannot save the action");
      setActions(next);
      const icon = await call("icon_from_program", { path: app.program }).catch(() => ({ error: "" }));
      item = { ...makeItem("button", x, y), label: app.name.slice(0, 24), actionId, ...("icon" in icon ? { icon: icon.icon } : {}) } as LayoutItem;
    }
    if (!item) return;
    const result = addItem(page!, layout!.grid, item);
    if (!result.ok) return setNotice(result.error);
    setPage(result.page);
    setSelectedId(item.id);
    setCreating(null);
  }

  return (
    <div className="stack">
      <h2>Layout</h2>

      <div className="row">
        <Field label="Columns">
          <select value={cols} onChange={(e) => setGrid({ cols: Number(e.target.value) })}>
            {options(COLS_RANGE).map((n) => (
              <option key={n} value={n}>
                {n}
              </option>
            ))}
          </select>
        </Field>
        <Field label="Rows">
          <select value={rows} onChange={(e) => setGrid({ rows: Number(e.target.value) })}>
            {options(ROWS_RANGE).map((n) => (
              <option key={n} value={n}>
                {n}
              </option>
            ))}
          </select>
        </Field>
        <span className="muted">Grid size. Items outside a smaller grid must be moved before saving.</span>
      </div>

      <div className="row wrap">
        <div className="page-tabs">
          {layout.pages.map((p, i) => (
            <button key={p.id} className={i === pageIdx ? "page-tab active" : "page-tab"} onClick={() => { setPageIdx(i); setSelectedId(null); setCreating(null); }}>
              {p.name}
              {p.popup === true && <span className="tag popup-tag">Popup</span>}
            </button>
          ))}
        </div>
        <Button onClick={addPage} disabled={layout.pages.length >= 10}>Add page</Button>
        <Button onClick={addPopupPage} disabled={layout.pages.length >= 10}>Add popup group</Button>
        <Button onClick={() => setTemplatesOpen(true)}>Add page from template</Button>
        <Button onClick={() => setRenaming(page.name)}>Rename</Button>
        <Button onClick={deletePage} disabled={layout.pages.length <= 1}>Delete page</Button>
        <Button onClick={() => shiftPage(-1)} disabled={pageIdx === 0} aria-label="Move page left">←</Button>
        <Button onClick={() => shiftPage(1)} disabled={pageIdx === layout.pages.length - 1} aria-label="Move page right">→</Button>
      </div>
      {renaming !== null && (
        <div className="row">
          <input autoFocus value={renaming} maxLength={30} onChange={(e) => setRenaming(e.target.value)} />
          <Button variant="primary" onClick={() => { setPage({ ...page, name: renaming }); setRenaming(null); }}>OK</Button>
          <Button onClick={() => setRenaming(null)}>Cancel</Button>
        </div>
      )}

      {page.popup === true ? (
        <section className="card">
          <h3>Popup group</h3>
          <p className="muted">A group opens over the page when a key with “Open popup group” is tapped. It is not part of the swipe and has no pages of its own to go to.</p>
          <label className="check">
            <input
              type="checkbox"
              checked={page.stayOpen === true}
              onChange={(e) => {
                const { stayOpen: _old, ...rest } = page;
                setPage(e.target.checked ? { ...rest, stayOpen: true } : rest);
              }}
            />{" "}
            Keep the group open after a key is pressed
          </label>
        </section>
      ) : (
        <PageApps page={page} isFirst={pageIdx === 0} onChange={setPage} />
      )}

      <div className="row wrap editor-toolbar">
        <Button onClick={() => step(undo)} disabled={!canUndo(hist!)} aria-label="Undo">↶ Undo</Button>
        <Button onClick={() => step(redo)} disabled={!canRedo(hist!)} aria-label="Redo">↷ Redo</Button>
        {dirty && <span className="unsaved">Unsaved changes</span>}
        {badItems.size > 0 && <span className="muted">Items with problems are listed below.</span>}
      </div>

      <div className="editor">
        <Palette actions={actions} />
        <div className="editor-main" ref={mainRef}>
          <EditorCanvas
            layout={layout}
            page={page}
            selectedId={selectedId}
            availableWidth={mainWidth - 16}
            onSelect={(id) => { setSelectedId(id); setCreating(null); }}
            onChangePage={setPage}
            onDropPayload={(payload, x, y) => void dropOnCell(payload, x, y)}
            onEmptyCell={(x, y) => { setCreating({ x, y }); setSelectedId(null); setNotice(null); }}
          />
          {notice && !creating && <p className="warn-text">{notice}</p>}

        <div className="form-pane">
          {creating && (
            <div className="card">
              <h3>New item at {creating.x},{creating.y}</h3>
              <div className="row wrap">
                {KINDS.map((k) => (
                  <Button key={k} onClick={() => create(k)}>
                    {`${k} ${DEFAULT_SIZE[k].w}×${DEFAULT_SIZE[k].h}`}
                  </Button>
                ))}
              </div>
              {notice && <p className="warn-text">{notice}</p>}
            </div>
          )}
          {selected && (
            <ItemForm
              item={selected}
              page={page}
              pages={layout.pages}
              grid={layout.grid}
              actions={actions}
              onChange={(patch) => setPage(updateItem(page, selected.id, patch))}
              onDelete={() => { setPage(removeItem(page, selected.id)); setSelectedId(null); }}
            />
          )}
          {!creating && !selected && <p className="muted">Drag programs, actions and widgets from the left onto a cell, drag keys to move or swap them, or click one to edit it.</p>}
        </div>
        </div>
      </div>

      <section className="card">
        <h3>Display</h3>
        <div className="grid-4">
          <Field label="Dim after, s" hint="0 = never">
            <input type="number" value={layout.display.dimAfterSec} onChange={(e) => setDisplay({ dimAfterSec: Number(e.target.value) })} />
          </Field>
          <Field label="Dim level" hint="0.05 to 1">
            <input type="number" step="0.05" value={layout.display.dimLevel} onChange={(e) => setDisplay({ dimLevel: Number(e.target.value) })} />
          </Field>
          <Field label="Screen saver after, s" hint="0 = never">
            <input type="number" value={layout.display.sleepAfterSec} onChange={(e) => setDisplay({ sleepAfterSec: Number(e.target.value) })} />
          </Field>
          <label className="check">
            <input type="checkbox" checked={layout.display.pixelShift} onChange={(e) => setDisplay({ pixelShift: e.target.checked })} /> Pixel shift
          </label>
        </div>
        <label className="check">
          <input type="checkbox" checked={layout.display.contextPages !== false} onChange={(e) => setDisplay({ contextPages: e.target.checked })} /> Switch to app pages automatically
        </label>
        <div className="row">
          <label className="check">
            <input type="checkbox" checked={layout.display.night.enabled} onChange={(e) => setDisplay({ night: { ...layout.display.night, enabled: e.target.checked } })} /> Night mode
          </label>
          <input aria-label="Night from" value={layout.display.night.from} placeholder="23:00" onChange={(e) => setDisplay({ night: { ...layout.display.night, from: e.target.value } })} />
          <span>to</span>
          <input aria-label="Night to" value={layout.display.night.to} placeholder="07:00" onChange={(e) => setDisplay({ night: { ...layout.display.night, to: e.target.value } })} />
        </div>
      </section>

      {shownIssues.length > 0 && (
        <Banner kind="error">
          <ul>
            {shownIssues.map((i, n) => (
              <li key={n}>
                {itemPath(i.path) ? (
                  <button type="button" className="link" onClick={() => selectIssue(i.path)}>
                    {describeIssue(i, layout)}
                  </button>
                ) : (
                  describeIssue(i, layout)
                )}
              </li>
            ))}
          </ul>
        </Banner>
      )}
      {message && <Banner kind="success">{message}</Banner>}

      <div className="row">
        <Button variant="primary" disabled={shownIssues.length > 0} onClick={() => void save()}>
          Save
        </Button>
        <Button onClick={() => void load()}>Revert</Button>
        {dirty && <span className="unsaved">Unsaved changes</span>}
        <span className="muted">Ctrl+S saves, Ctrl+Z / Ctrl+Y undo and redo.</span>
      </div>
      {templatesOpen && (
        <TemplatesDialog
          onClose={() => setTemplatesOpen(false)}
          onAdded={() => {
            setTemplatesOpen(false);
            // The new page is the last one; open it.
            load().then((fresh) => setPageIdx(fresh.pages.length - 1), (e) => setError(String(e)));
          }}
        />
      )}
    </div>
  );
}
