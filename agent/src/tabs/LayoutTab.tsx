import { useCallback, useEffect, useMemo, useState } from "react";
import type { ActionsFile, DisplaySettings, LayoutFile, LayoutItem, ValidationIssue } from "@decka/shared";
import { validateLayout } from "@decka/shared";
import { call } from "../ipc";
import Banner from "../ui/Banner";
import Button from "../ui/Button";
import Field from "../ui/Field";
import ItemForm from "./itemForm";
import { addItem, COLS, DEFAULT_SIZE, makeItem, movePage, newPageId, removeItem, ROWS, updateItem, type Kind } from "./layoutOps";

const KINDS: Kind[] = ["button", "clock", "sys", "status", "statuses", "notifications"];
const CELL_W = 120;
const CELL_H = 90;

/** "/pages/1/items/3" -> [1, 3]; anything else -> null. */
function itemPath(path: string): [number, number] | null {
  const m = /^\/pages\/(\d+)\/items\/(\d+)/.exec(path);
  return m ? [Number(m[1]), Number(m[2])] : null;
}

function itemLabel(item: LayoutItem): string {
  if (item.kind === "button") return item.label ?? item.icon?.replace(/^lucide:/, "") ?? item.id;
  if (item.kind === "status") return item.statusId || "status";
  return item.kind;
}

export default function LayoutTab() {
  const [layout, setLayout] = useState<LayoutFile | null>(null);
  const [actions, setActions] = useState<ActionsFile | null>(null);
  const [pageIdx, setPageIdx] = useState(0);
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [creating, setCreating] = useState<{ x: number; y: number } | null>(null);
  const [renaming, setRenaming] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const [serverIssues, setServerIssues] = useState<ValidationIssue[]>([]);
  const [message, setMessage] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  const load = useCallback(async () => {
    setLayout(await call("get_layout"));
    setActions(await call("get_actions"));
    setServerIssues([]);
    setMessage(null);
    setSelectedId(null);
    setCreating(null);
  }, []);

  useEffect(() => {
    load().catch((e) => setError(String(e)));
  }, [load]);

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
    setLayout(next);
    setServerIssues([]);
    setMessage(null);
  };
  const setPage = (p: typeof page) => edit({ ...layout, pages: layout.pages.map((x, i) => (i === pageIdx ? p : x)) });
  const setDisplay = (d: Partial<DisplaySettings>) => edit({ ...layout, display: { ...layout.display, ...d } });

  function create(kind: Kind) {
    if (!creating || !page) return;
    const result = addItem(page, makeItem(kind, creating.x, creating.y));
    if (!result.ok) return setNotice(result.error);
    const added = result.page.items[result.page.items.length - 1];
    setNotice(null);
    setPage(result.page);
    setCreating(null);
    setSelectedId(added?.id ?? null);
  }

  function addPage() {
    const n = layout!.pages.length + 1;
    edit({ ...layout!, pages: [...layout!.pages, { id: newPageId(), name: `Page ${n}`, items: [] }] });
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
    const result = await call("save_layout", { layout: layout! });
    if (result.ok) {
      setServerIssues([]);
      setMessage("Saved");
    } else {
      setServerIssues(result.issues);
      setMessage(null);
    }
  }

  // Which cells are covered by an item.
  const covered = new Set<string>();
  for (const it of page.items) for (let dx = 0; dx < it.w; dx++) for (let dy = 0; dy < it.h; dy++) covered.add(`${it.x + dx},${it.y + dy}`);

  return (
    <div className="stack">
      <h2>Layout</h2>

      <div className="row wrap">
        <div className="page-tabs">
          {layout.pages.map((p, i) => (
            <button key={p.id} className={i === pageIdx ? "page-tab active" : "page-tab"} onClick={() => { setPageIdx(i); setSelectedId(null); setCreating(null); }}>
              {p.name}
            </button>
          ))}
        </div>
        <Button onClick={addPage} disabled={layout.pages.length >= 10}>Add page</Button>
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

      <div className="split">
        <div>
          <div className="layout-grid" style={{ gridTemplateColumns: `repeat(${COLS}, ${CELL_W}px)`, gridTemplateRows: `repeat(${ROWS}, ${CELL_H}px)` }}>
            {Array.from({ length: ROWS }, (_, y) =>
              Array.from({ length: COLS }, (_, x) =>
                covered.has(`${x},${y}`) ? null : (
                  <button key={`e${x}${y}`} className="cell empty" style={{ gridColumn: x + 1, gridRow: y + 1 }} aria-label={`Add item at ${x},${y}`} onClick={() => { setCreating({ x, y }); setSelectedId(null); setNotice(null); }}>
                    +
                  </button>
                ),
              ),
            )}
            {page.items.map((it) => (
              <button
                key={it.id}
                className={`cell item${it.id === selectedId ? " selected" : ""}${badItems.has(it.id) ? " bad" : ""}`}
                style={{ gridColumn: `${it.x + 1} / span ${it.w}`, gridRow: `${it.y + 1} / span ${it.h}`, background: it.kind === "button" ? (it.color ?? undefined) : undefined }}
                onClick={() => { setSelectedId(it.id); setCreating(null); }}
              >
                <span className="muted">{it.kind}</span>
                <strong>{itemLabel(it)}</strong>
              </button>
            ))}
          </div>
        </div>

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
              actions={actions}
              onChange={(patch) => setPage(updateItem(page, selected.id, patch))}
              onDelete={() => { setPage(removeItem(page, selected.id)); setSelectedId(null); }}
            />
          )}
          {!creating && !selected && <p className="muted">Click an empty cell to add an item, or an item to edit it.</p>}
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
                <code>{i.path || "/"}</code>: {i.message}
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
      </div>
    </div>
  );
}
