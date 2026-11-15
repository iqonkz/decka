import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import type { ActionsFile, LayoutFile, ValidationIssue } from "@decka/shared";
import { validateLayout } from "@decka/shared";
import { call } from "../ipc";
import Banner from "../ui/Banner";
import Button from "../ui/Button";
import Field from "../ui/Field";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import type { DragPayload } from "./dragMachine";
import { canRedo, canUndo, itemAt, moveItem, nearestFreeCell, newHistory, pushHistory, redo, undo, type History } from "./editorOps";
import EditorCanvas from "./EditorCanvas";
import { useDragSession, type Cell } from "./useDragSession";
import Palette from "./Palette";
import ItemForm from "./itemForm";
import { describeIssue, itemPath } from "./issueText";
import { COLS_RANGE, movePage, newPageId, newPopupPage, removeItem, ROWS_RANGE, updateItem } from "./layoutOps";
import { addButtonAt, type Choice } from "../editor/addButton";
import QuickPicker from "../editor/QuickPicker";
import PageApps from "./PageApps";
import TemplatesDialog from "./TemplatesDialog";

const sameLayout = (a: LayoutFile, b: LayoutFile) => JSON.stringify(a) === JSON.stringify(b);

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
  // The quick picker (DESIGN.md 4.5): the empty cell that was clicked and where it is on screen.
  const [creating, setCreating] = useState<{ x: number; y: number; rect: DOMRect } | null>(null);
  const [renaming, setRenaming] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const [serverIssues, setServerIssues] = useState<ValidationIssue[]>([]);
  const [message, setMessage] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [templatesOpen, setTemplatesOpen] = useState(false);

  // Drag and drop on pointer events (D-38) plus files dropped from Explorer (Tauri). The handlers are set below, once the data is loaded.
  const cellAtRef = useRef<(clientX: number, clientY: number) => Cell | null>(() => null);
  const dropRef = useRef<(payload: DragPayload, cell: Cell) => void>(() => {});
  const clickRef = useRef<(payload: DragPayload) => void>(() => {});
  const filesRef = useRef<(paths: string[], cell: Cell | null) => void>(() => {});
  const [fileHover, setFileHover] = useState<Cell | null>(null);
  const session = useDragSession({
    cellAt: (x, y) => cellAtRef.current(x, y),
    onDrop: (payload, cell) => dropRef.current(payload, cell),
    onClick: (payload) => clickRef.current(payload),
  });
  useEffect(() => {
    let dead = false;
    let unlisten: (() => void) | undefined;
    try {
      void getCurrentWebview()
        .onDragDropEvent((event) => {
          const p = event.payload;
          if (p.type === "leave") return setFileHover(null);
          const dpr = window.devicePixelRatio || 1;
          const cell = cellAtRef.current(p.position.x / dpr, p.position.y / dpr);
          if (p.type === "drop") {
            setFileHover(null);
            filesRef.current(p.paths, cell);
          } else setFileHover(cell);
        })
        .then((fn) => (dead ? fn() : (unlisten = fn)))
        .catch(() => {});
    } catch {
      // not running inside the Tauri window (tests): no Explorer drops
    }
    return () => {
      dead = true;
      unlisten?.();
    };
  }, []);

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
  // Items are never moved by a size change: those that no longer fit are flagged by validation (red) and block Save.
  const setGrid = (g: Partial<LayoutFile["grid"]>) => edit({ ...layout, grid: { ...layout.grid, ...g } });
  const { cols, rows } = layout.grid;
  const options = (range: readonly [number, number]) => Array.from({ length: range[1] - range[0] + 1 }, (_, i) => range[0] + i);

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

  /**
   * Puts `choice` on cell (x, y) of the current page (DESIGN.md 4.5): takes the program's icon, saves a new action first,
   * then returns the new layout and actions for the caller to apply.
   */
  async function addChoice(lay: LayoutFile, acts: ActionsFile, cell: { x: number; y: number }, choice: Choice): Promise<{ layout: LayoutFile; actions: ActionsFile; itemId: string } | { error: string }> {
    let c = choice;
    if (c.type === "app" && c.icon === undefined) {
      const icon = await call("icon_from_program", { path: c.app.program }).catch(() => ({ error: "" }));
      if ("icon" in icon) c = { ...c, icon: icon.icon };
    }
    const made = addButtonAt(lay, acts, pageIdx, cell, c);
    if (!made.ok) return { error: made.error };
    if (made.actionsChanged) {
      const saved = await call("save_actions", { actions: made.actions });
      if (!saved.ok) return { error: saved.issues[0]?.message ?? "Cannot save the action" };
    }
    return { layout: made.layout, actions: made.actions, itemId: made.itemId };
  }

  /** The quick picker's choice becomes a button on the clicked cell, selected, with its properties open. */
  async function choose(choice: Choice) {
    if (!creating) return;
    setNotice(null);
    const made = await addChoice(layout!, actions!, creating, choice);
    if ("error" in made) return setNotice(made.error);
    setActions(made.actions);
    edit(made.layout);
    setSelectedId(made.itemId);
    setCreating(null);
  }

  /** `.exe` / `.lnk` files dropped from Explorer: each becomes a button in the cell under the pointer or the nearest free one. */
  async function dropFiles(paths: string[], cell: Cell | null) {
    if (!cell || !page) return;
    setNotice(null);
    let lay: LayoutFile = layout!;
    let acts: ActionsFile = actions!;
    let lastId: string | null = null;
    for (const path of paths) {
      const app = await call("describe_program", { path });
      if ("error" in app) {
        setNotice(`${path.split(/[\\/]/).pop()}: ${app.error}`);
        continue;
      }
      const current = lay.pages[pageIdx] ?? page;
      const spot = nearestFreeCell(current, lay.grid, cell.x, cell.y);
      if (!spot) {
        setNotice("No free cell on this page");
        break;
      }
      const made = await addChoice(lay, acts, spot, { type: "app", app });
      if ("error" in made) {
        setNotice(made.error);
        break;
      }
      lay = made.layout;
      acts = made.actions;
      lastId = made.itemId;
    }
    if (lastId) {
      setActions(acts);
      edit(lay);
      setSelectedId(lastId);
      setCreating(null);
    }
  }

  /** Something was dropped on cell (x, y): a key moved or swapped, or a program, an action or a widget from the palette becomes an item. */
  async function dropOnCell(payload: DragPayload, x: number, y: number) {
    setNotice(null);
    if (payload.type === "item") {
      const moved = moveItem(page!, layout!.grid, payload.id, x, y);
      if (moved) setPage(moved);
      return;
    }
    if (payload.type === "app" && itemAt(page!, x, y)) return setNotice("Drop a program on an empty cell.");
    const choice: Choice = payload.type === "widget" ? { type: "widget", kind: payload.kind } : payload.type === "action" ? { type: "action", actionId: payload.actionId } : { type: "app", app: payload.app };
    const made = await addChoice(layout!, actions!, { x, y }, choice);
    if ("error" in made) return setNotice(made.error);
    setActions(made.actions);
    edit(made.layout);
    setSelectedId(made.itemId);
    setCreating(null);
  }
  dropRef.current = (payload, cell) => void dropOnCell(payload, cell.x, cell.y);
  clickRef.current = (payload) => {
    if (payload.type !== "item") return;
    setSelectedId(payload.id);
    setCreating(null);
  };
  filesRef.current = (paths, cell) => void dropFiles(paths, cell);
  const ghostLabel = (payload: DragPayload): string => {
    if (payload.type === "app") return payload.app.name;
    if (payload.type === "action") return payload.label;
    if (payload.type === "widget") return payload.kind;
    const item = page.items.find((i) => i.id === payload.id);
    if (item && "label" in item && item.label) return item.label;
    const action = item && item.kind === "button" && item.actionId ? actions.actions[item.actionId] : undefined;
    return action?.name ?? item?.kind ?? "";
  };

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
        <Palette actions={actions} onBegin={session.begin} />
        <div className="editor-main" ref={mainRef}>
          <EditorCanvas
            layout={layout}
            page={page}
            selectedId={selectedId}
            availableWidth={mainWidth - 16}
            onSelect={(id) => { setSelectedId(id); setCreating(null); }}
            onChangePage={setPage}
            hover={session.hover ?? fileHover}
            draggingId={session.dragging?.payload.type === "item" ? session.dragging.payload.id : null}
            cellAtRef={cellAtRef}
            onItemPointerDown={session.begin}
            onEmptyCell={(x, y, rect) => { setCreating({ x, y, rect }); setSelectedId(null); setNotice(null); }}
          />
          {notice && !creating && <p className="warn-text">{notice}</p>}

        <div className="form-pane">
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
          {!selected && <p className="muted">Click an empty cell to add an app or action, drag programs from the left, drag keys to move or swap them, or click a key to edit it.</p>}
        </div>
        </div>
      </div>

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
      {creating && (
        <QuickPicker anchor={creating.rect} cell={creating} page={page} grid={layout.grid} actions={actions} onChoose={(c) => void choose(c)} onClose={() => setCreating(null)} />
      )}
      {session.dragging && (
        <div className="drag-ghost" style={{ left: session.dragging.x + 12, top: session.dragging.y + 12 }}>
          {ghostLabel(session.dragging.payload)}
        </div>
      )}
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
