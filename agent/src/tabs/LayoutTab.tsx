import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import type { ActionsFile, ButtonItem, LayoutFile, LayoutItem, Page, ValidationIssue } from "@decka/shared";
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
import { COLS_RANGE, DEFAULT_SIZE, fits, movePage, newItemId, newPageId, newPopupPage, removeItem, ROWS_RANGE, sizeRange, tapModePatch, updateItem } from "./layoutOps";
import { resizeItem } from "./editorOps";
import ContextMenu, { suppressNativeMenu, type MenuItem } from "../editor/ContextMenu";
import { describeAction } from "../editor/describeAction";
import { addButtonAt, type Choice } from "../editor/addButton";
import QuickPicker from "../editor/QuickPicker";
import EditorTips from "../editor/EditorTips";
import PhoneStrip from "../editor/PhoneStrip";
import PageApps from "./PageApps";
import GridPicker from "../editor/GridPicker";
import { overflowItems, resizeGridMoving } from "../editor/gridResize";
import PhIcon from "../ui/PhIcon";
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
  // `replaceId`: the picker changes the action of that button instead of adding one.
  const [creating, setCreating] = useState<{ x: number; y: number; rect: DOMRect; replaceId?: string } | null>(null);
  const [renaming, setRenaming] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const [serverIssues, setServerIssues] = useState<ValidationIssue[]>([]);
  const [message, setMessage] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [templatesOpen, setTemplatesOpen] = useState(false);
  const [resizing, setResizing] = useState<{ cols: number; rows: number; count: number } | null>(null);
  // Autosave (DESIGN.md 4.6): `saved` / `savedActions` are what is on the PC; 600 ms after the last edit the difference is written.
  const [savedActions, setSavedActions] = useState("");
  const [savedFlash, setSavedFlash] = useState(false);
  const [tick, setTick] = useState(0);
  const savingRef = useRef(false);
  const [menu, setMenu] = useState<{ x: number; y: number; items: MenuItem[] } | null>(null);
  const [renamingId, setRenamingId] = useState<string | null>(null);
  const [clipboard, setClipboard] = useState<LayoutItem | null>(null);
  const keysRef = useRef<(e: KeyboardEvent) => void>(() => {});

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
    const acts = await call("get_actions");
    setActions(acts);
    setSavedActions(JSON.stringify(acts));
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

  // The window has its own menus: no WebView2 menu (`Back`, `Refresh`, `Inspect`) over them.
  useEffect(() => suppressNativeMenu(), []);

  // Ctrl+Z / Ctrl+Y / Ctrl+S, Delete, F2, Ctrl+D, Ctrl+C / Ctrl+V (text fields keep their own keys).
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => keysRef.current(e);
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, []);

  useEffect(() => {
    if (!savedFlash) return;
    const timer = window.setTimeout(() => setSavedFlash(false), 1500);
    return () => window.clearTimeout(timer);
  }, [savedFlash]);
  const issues = useMemo(() => (layout && actions ? validateLayout(layout, actions) : []), [layout, actions]);
  const shownIssues = [...issues, ...serverIssues.filter((s) => !issues.some((i) => i.path === s.path && i.message === s.message))];

  const dirty = layout !== null && actions !== null && (JSON.stringify(layout) !== saved || JSON.stringify(actions) !== savedActions);
  useEffect(() => {
    if (!dirty || shownIssues.length > 0) return;
    const timer = window.setTimeout(() => saveRef.current(), 600);
    return () => window.clearTimeout(timer);
  }, [dirty, layout, actions, tick]); // eslint-disable-line react-hooks/exhaustive-deps

  if (error) return <Banner kind="error">{error}</Banner>;
  if (!layout || !actions) return <p>Loading…</p>;

  const firstPage = layout.pages[pageIdx] ?? layout.pages[0];
  if (!firstPage) return <Banner kind="error">The layout has no pages.</Banner>;
  const page: Page = firstPage; // a plain `Page` so that the helpers below need no checks
  const selected = page.items.find((i) => i.id === selectedId) ?? null;
  const badItems = new Set(
    shownIssues.map((i) => itemPath(i.path)).filter((p): p is [number, number] => p !== null && p[0] === pageIdx).map(([, n]) => page.items[n]?.id),
  );

  const edit = (next: LayoutFile) => {
    setHist((h) => (h ? pushHistory(h, next, sameLayout) : h));
    setServerIssues([]);
    setMessage(null);
  };
  const step = (change: (h: History<LayoutFile>) => History<LayoutFile>) => {
    setHist((h) => (h ? change(h) : h));
    setServerIssues([]);
    setMessage(null);
  };
  const setPage = (p: typeof page) => edit({ ...layout, pages: layout.pages.map((x, i) => (i === pageIdx ? p : x)) });
  // Items are never moved by a size change: those that no longer fit are flagged by validation (red) and block Save.
  const setGrid = (g: Partial<LayoutFile["grid"]>) => edit({ ...layout, grid: { ...layout.grid, ...g } });
  const { cols, rows } = layout.grid;
  /** A new grid size: applied at once when nothing sticks out, else after the `Move them?` question (DESIGN.md 4.6a). */
  function requestGrid(c: number, r: number) {
    if (c === cols && r === rows) return;
    const stuck = overflowItems(layout!, c, r);
    if (stuck.length === 0) return setGrid({ cols: c, rows: r });
    setResizing({ cols: c, rows: r, count: stuck.length });
  }
  function confirmResize() {
    if (!resizing) return;
    const result = resizeGridMoving(layout!, resizing.cols, resizing.rows);
    setResizing(null);
    if (result.lost > 0) return setNotice(`${result.lost} widget${result.lost === 1 ? "" : "s"} are too big for this grid and were not moved.`);
    edit(result.layout);
    setSelectedId(null);
  }
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

  /** Writes what changed: the actions first, then the layout (DESIGN.md 4.6). An error stays at the fields; the history is kept. */
  async function save() {
    if (savingRef.current || shownIssues.length > 0) return;
    const layoutNow = layout!;
    const actionsNow = actions!;
    savingRef.current = true;
    try {
      if (JSON.stringify(actionsNow) !== savedActions) {
        const r = await call("save_actions", { actions: actionsNow });
        if (!r.ok) return setServerIssues(r.issues);
        setSavedActions(JSON.stringify(actionsNow));
      }
      if (JSON.stringify(layoutNow) !== saved) {
        const result = await call("save_layout", { layout: layoutNow });
        if (!result.ok) return setServerIssues(result.issues);
        setServerIssues([]);
        setSaved(JSON.stringify(layoutNow));
      }
      setSavedFlash(true);
    } catch (e) {
      setNotice(String(e));
    } finally {
      savingRef.current = false;
      setTick((n) => n + 1); // edits made while saving are picked up
    }
  }  saveRef.current = () => void save();

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

  /** The quick picker's choice becomes a button on the clicked cell, selected, with its properties open; or the new action of a button. */
  async function choose(choice: Choice) {
    if (!creating) return;
    setNotice(null);
    if (creating.replaceId) {
      const old = page!.items.find((i) => i.id === creating.replaceId);
      if (!old || old.kind !== "button") return setCreating(null);
      // Build the choice on a page without the old key, then move its action (and its name, when that was automatic) over.
      const without: LayoutFile = { ...layout!, pages: layout!.pages.map((p, i) => (i === pageIdx ? removeItem(p, old.id) : p)) };
      const made = await addChoice(without, actions!, { x: old.x, y: old.y }, choice);
      if ("error" in made) return setNotice(made.error);
      const fresh = made.layout.pages[pageIdx]!.items.at(-1) as ButtonItem;
      const oldAction = old.actionId ? actions!.actions[old.actionId] : undefined;
      const automatic = !old.label || old.label === oldAction?.name.slice(0, 24);
      applyActions(made.actions);
      setPage(updateItem(page!, old.id, { actionId: fresh.actionId, ...(automatic ? { label: fresh.label, icon: fresh.icon } : {}), ...(fresh.confirm ? { confirm: true } : {}) } as Partial<LayoutItem>));
      setSelectedId(old.id);
      return setCreating(null);
    }
    const made = await addChoice(layout!, actions!, creating, choice);
    if ("error" in made) return setNotice(made.error);
    applyActions(made.actions);
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
      applyActions(acts);
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
    applyActions(made.actions);
    edit(made.layout);
    setSelectedId(made.itemId);
    setCreating(null);
  }
  const applyActions = (a: ActionsFile) => {
    setActions(a);
    setSavedActions(JSON.stringify(a)); // addChoice saved them already
  };
  const patchItem = (id: string, patch: Record<string, unknown>) => setPage(updateItem(page, id, patch as Partial<LayoutItem>));
  const deleteItem = (id: string) => {
    setPage(removeItem(page, id));
    setSelectedId(null);
  };
  function duplicateItem(id: string) {
    const item = page.items.find((i) => i.id === id);
    if (!item) return;
    const spot = nearestFreeCell(page, layout!.grid, item.x, item.y, item.w, item.h);
    if (!spot) return setNotice("No free cell on this page");
    const copy = { ...item, id: newItemId(item.kind), x: spot.x, y: spot.y } as LayoutItem;
    setPage({ ...page, items: [...page.items, copy] });
    setSelectedId(copy.id);
  }
  function pasteAt(cell: { x: number; y: number } | null) {
    if (!clipboard) return;
    const spot = nearestFreeCell(page, layout!.grid, cell?.x ?? clipboard.x, cell?.y ?? clipboard.y, clipboard.w, clipboard.h);
    if (!spot) return setNotice("No free cell on this page");
    const copy = { ...clipboard, id: newItemId(clipboard.kind), x: spot.x, y: spot.y } as LayoutItem;
    setPage({ ...page, items: [...page.items, copy] });
    setSelectedId(copy.id);
  }
  function makePopup(id: string) {
    const item = page.items.find((i) => i.id === id);
    if (!item || item.kind !== "button" || page.popup === true) return;
    const group = newPopupPage(layout!.pages);
    const patch = tapModePatch(item, item.actionId !== undefined ? "action-popup" : "popup", { firstActionId: undefined, firstPageId: undefined, firstPopupId: group.id });
    edit({ ...layout!, pages: [...layout!.pages.map((p, i) => (i === pageIdx ? updateItem(p, id, patch as Partial<LayoutItem>) : p)), group] });
    setSelectedId(id);
  }
  function goToPage(pageId: string | undefined) {
    const i = layout!.pages.findIndex((p) => p.id === pageId);
    if (i >= 0) {
      setPageIdx(i);
      setSelectedId(null);
    }
  }
  async function testItem(item: ButtonItem) {
    if (!item.actionId) return;
    const r = await call("test_action", { actionId: item.actionId }).catch((e) => ({ ok: false, error: String(e) }));
    setNotice(r.ok ? "Sent to the PC" : `Test failed: ${"error" in r && r.error ? r.error : "unknown error"}`);
  }
  /** The program a button starts, when its action launches one (the `Refresh app icon` item is only for these). */
  const launchProgramOf = (item: ButtonItem): string | null => {
    const a = item.actionId ? actions!.actions[item.actionId] : undefined;
    return a?.type === "launch" ? a.program : null;
  };
  async function refreshIcon(id: string, program: string) {
    const r = await call("icon_from_program", { path: program }).catch(() => ({ error: "" }));
    if ("icon" in r) patchItem(id, { icon: r.icon });
    else setNotice("Cannot read the icon of this program");
  }
  function pickCustomColor(item: ButtonItem) {
    const input = document.createElement("input");
    input.type = "color";
    input.value = item.color ?? "#7c5cff";
    input.addEventListener("input", () => patchItem(item.id, { color: input.value }));
    input.click();
  }
  const COLORS = ["#7c5cff", "#3b82f6", "#06b6d4", "#16a34a", "#eab308", "#f97316", "#ef4444", "#ec4899"];
  const WIDGET_NAMES = { clock: "Clock", sys: "System", media: "Now playing", statuses: "Statuses", notifications: "Notifications" } as const;

  /** The context menu of a key or widget (DESIGN.md 4.6). */
  function itemMenu(item: LayoutItem, x: number, y: number) {
    setSelectedId(item.id);
    setCreating(null);
    let items: MenuItem[];
    if (item.kind === "button") {
      items = [
        { label: "Change action…", icon: "lightning", onClick: () => setCreating({ x: item.x, y: item.y, rect: new DOMRect(x, y, 0, 0), replaceId: item.id }) },
        { label: "Change icon…", icon: "image", onClick: () => document.querySelector(".item-form")?.scrollIntoView({ block: "start" }) },
        { label: "Rename", icon: "pencil-simple", hint: "F2", onClick: () => setRenamingId(item.id) },
        {
          label: "Color",
          icon: "palette",
          separator: true,
          submenu: [
            ...COLORS.map((c) => ({ label: c, swatch: c, checked: item.color?.toLowerCase() === c, onClick: () => patchItem(item.id, { color: c }) })),
            { label: "Custom…", onClick: () => pickCustomColor(item) },
            { label: "Default", onClick: () => patchItem(item.id, { color: undefined }) },
          ],
        },
        { label: "Big icon", icon: "frame-corners", separator: true, checked: (item.face ?? layout!.display.keyFace ?? "standard") === "full", onClick: () => patchItem(item.id, { face: (item.face ?? layout!.display.keyFace ?? "standard") === "full" ? "standard" : "full" }) },
        { label: "Show name", icon: "text-t", checked: item.showLabel ?? layout!.display.keyLabels ?? true, onClick: () => patchItem(item.id, { showLabel: !(item.showLabel ?? layout!.display.keyLabels ?? true) }) },
        { label: "Ask before running", icon: "hand-palm", checked: item.confirm === true, onClick: () => patchItem(item.id, { confirm: item.confirm === true ? undefined : true }) },
        { label: "Test on PC", icon: "play", separator: true, disabled: !item.actionId, onClick: () => void testItem(item) },
        { label: "Duplicate", icon: "copy", hint: "Ctrl+D", onClick: () => duplicateItem(item.id) },
        item.popup !== undefined
          ? { label: "Edit popup group", icon: "squares-four", onClick: () => goToPage(item.popup) }
          : { label: "Make popup group", icon: "squares-four", disabled: page.popup === true, onClick: () => makePopup(item.id) },
        ...(launchProgramOf(item) !== null ? [{ label: "Refresh app icon", icon: "arrows-clockwise", onClick: () => void refreshIcon(item.id, launchProgramOf(item)!) }] : []),
        { label: "Delete", icon: "trash", hint: "Del", danger: true, separator: true, onClick: () => deleteItem(item.id) },
      ];
    } else {
      const [minW, maxW, minH, maxH] = sizeRange(item.kind, layout!.grid);
      const sizes: MenuItem[] = [];
      for (let w = minW; w <= maxW; w++)
        for (let h = minH; h <= maxH; h++) {
          sizes.push({
            label: `${w}×${h}`,
            checked: w === item.w && h === item.h,
            onClick: () => {
              const next = resizeItem(page, layout!.grid, item.id, w, h);
              if (next) setPage(next);
              else setNotice("Not enough free space for this size");
            },
          });
        }
      items = [
        { label: "Settings…", icon: "gear", onClick: () => document.querySelector(".item-form")?.scrollIntoView({ block: "start" }) },
        ...(sizes.length > 1 ? [{ label: "Size", icon: "arrows-out", submenu: sizes }] : []),
        { label: "Duplicate", icon: "copy", hint: "Ctrl+D", onClick: () => duplicateItem(item.id) },
        { label: "Delete", icon: "trash", hint: "Del", danger: true, separator: true, onClick: () => deleteItem(item.id) },
      ];
    }
    setMenu({ x, y, items });
  }

  async function addWidget(kind: keyof typeof WIDGET_NAMES, cell: { x: number; y: number }) {
    const made = await addChoice(layout!, actions!, cell, { type: "widget", kind });
    if ("error" in made) return setNotice(made.error);
    applyActions(made.actions);
    edit(made.layout);
    setSelectedId(made.itemId);
  }

  /** The context menu of an empty cell. */
  function cellMenu(cell: { x: number; y: number }, x: number, y: number, rect: DOMRect) {
    setSelectedId(null);
    const fitting = (Object.keys(WIDGET_NAMES) as (keyof typeof WIDGET_NAMES)[]).filter((kind) => fits(page, layout!.grid, { x: cell.x, y: cell.y, w: DEFAULT_SIZE[kind].w, h: DEFAULT_SIZE[kind].h }));
    setMenu({
      x,
      y,
      items: [
        { label: "Add button…", icon: "plus", onClick: () => setCreating({ ...cell, rect }) },
        { label: "Add widget", icon: "squares-four", disabled: fitting.length === 0, submenu: fitting.map((kind) => ({ label: WIDGET_NAMES[kind], onClick: () => void addWidget(kind, cell) })) },
        { label: "Paste", icon: "clipboard", hint: "Ctrl+V", disabled: clipboard === null, onClick: () => pasteAt(cell) },
      ],
    });
  }

  /** The context menu of a page tab. */
  function pageMenu(index: number, x: number, y: number) {
    setPageIdx(index);
    setSelectedId(null);
    const target = layout!.pages[index]!;
    const many = layout!.pages.length;
    setMenu({
      x,
      y,
      items: [
        { label: "Rename", icon: "pencil-simple", onClick: () => setRenaming(target.name) },
        { label: "Move left", icon: "arrow-left", disabled: index === 0, onClick: () => { edit(movePage(layout!, index, -1)); setPageIdx(index - 1); } },
        { label: "Move right", icon: "arrow-right", disabled: index === many - 1, onClick: () => { edit(movePage(layout!, index, 1)); setPageIdx(index + 1); } },
        {
          label: "Duplicate",
          icon: "copy",
          disabled: many >= 10,
          onClick: () => {
            const copy: Page = { ...target, id: newPageId(), name: `${target.name} copy`.slice(0, 30), items: target.items.map((i) => ({ ...i, id: newItemId(i.kind) }) as LayoutItem) };
            edit({ ...layout!, pages: [...layout!.pages.slice(0, index + 1), copy, ...layout!.pages.slice(index + 1)] });
            setPageIdx(index + 1);
          },
        },
        { label: "Delete", icon: "trash", danger: true, separator: true, disabled: many <= 1, onClick: () => { setPageIdx(index); deletePage(); } },
      ],
    });
  }

  keysRef.current = (e) => {
    const target = e.target;
    const typing = target instanceof HTMLElement && (/^(INPUT|TEXTAREA|SELECT)$/.test(target.tagName) || target.isContentEditable);
    const ctrl = e.ctrlKey || e.metaKey;
    const key = e.key.toLowerCase();
    if (ctrl && key === "s") {
      e.preventDefault();
      return saveRef.current();
    }
    if (typing || menu || creating || renamingId) return;
    if (ctrl && key === "z") {
      e.preventDefault();
      step(e.shiftKey ? redo : undo);
    } else if (ctrl && key === "y") {
      e.preventDefault();
      step(redo);
    } else if (ctrl && key === "d" && selected) {
      e.preventDefault();
      duplicateItem(selected.id);
    } else if (ctrl && key === "c" && selected) {
      setClipboard(selected);
    } else if (ctrl && key === "v" && clipboard) {
      e.preventDefault();
      pasteAt(null);
    } else if (e.key === "Delete" && selected) {
      e.preventDefault();
      deleteItem(selected.id);
    } else if (e.key === "F2" && selected?.kind === "button") {
      e.preventDefault();
      setRenamingId(selected.id);
    }
  };
  dropRef.current = (payload, cell) => void dropOnCell(payload, cell.x, cell.y);
  clickRef.current = (payload) => {
    if (payload.type !== "item") return;
    setSelectedId(payload.id);
    setCreating(null);
  };
  filesRef.current = (paths, cell) => void dropFiles(paths, cell);
  // A key dragged over a taken cell: show the page as it will be after the swap, so the neighbour glides over (and back when the key leaves).
  const dragged = session.dragging?.payload;
  const hoverCell = session.hover;
  const swapPreview: Page | null =
    dragged?.type === "item" && hoverCell && itemAt(page, hoverCell.x, hoverCell.y) && itemAt(page, hoverCell.x, hoverCell.y)!.id !== dragged.id
      ? moveItem(page, layout.grid, dragged.id, hoverCell.x, hoverCell.y)
      : null;
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
      <h2>Deck</h2>

      <div className="row wrap deck-bar">
        <div className="page-tabs">
          {layout.pages.map((p, i) =>
            renaming !== null && i === pageIdx ? (
              <input
                key={p.id}
                className="page-tab-input"
                aria-label="Page name"
                autoFocus
                maxLength={30}
                value={renaming}
                onChange={(e) => setRenaming(e.target.value)}
                onFocus={(e) => e.currentTarget.select()}
                onBlur={() => {
                  if (renaming.trim() !== "") setPage({ ...page, name: renaming.trim() });
                  setRenaming(null);
                }}
                onKeyDown={(e) => {
                  if (e.key === "Enter") e.currentTarget.blur();
                  else if (e.key === "Escape") setRenaming(null);
                }}
              />
            ) : (
              <button
                key={p.id}
                className={i === pageIdx ? "page-tab active" : "page-tab"}
                onClick={() => { setPageIdx(i); setSelectedId(null); setCreating(null); }}
                onDoubleClick={() => { setPageIdx(i); setRenaming(p.name); }}
                onContextMenu={(e) => pageMenu(i, e.clientX, e.clientY)}
              >
                {p.name}
                {p.popup === true && <span className="tag popup-tag">Popup</span>}
              </button>
            ),
          )}
          <button
            type="button"
            className="page-tab"
            aria-label="Add a page"
            disabled={layout.pages.length >= 10}
            onClick={(e) => {
              const box = e.currentTarget.getBoundingClientRect();
              setMenu({
                x: box.left,
                y: box.bottom + 4,
                items: [
                  { label: "New page", icon: "file-plus", onClick: addPage },
                  { label: "New popup group", icon: "squares-four", onClick: addPopupPage },
                  { label: "From a template…", icon: "layout", onClick: () => setTemplatesOpen(true) },
                ],
              });
            }}
          >
            +
          </button>
        </div>
        <GridPicker cols={cols} rows={rows} onPick={requestGrid} />
      </div>

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
      {badItems.size > 0 && <p className="muted">Keys with problems are listed below.</p>}
      <div className="editor">
        <Palette actions={actions} onBegin={session.begin} />
        <div className="editor-main" ref={mainRef} style={{ position: "relative" }}>
          <div className="preview-bar">
            <div className="grow">
              <PhoneStrip />
            </div>
            {savedFlash && <span className="saved-flag">Saved</span>}
            <button type="button" className="icon-btn" aria-label="Undo" title="Undo (Ctrl+Z)" disabled={!canUndo(hist!)} onClick={() => step(undo)}>
              <PhIcon name="arrow-counter-clockwise" size={16} />
            </button>
            <button type="button" className="icon-btn" aria-label="Redo" title="Redo (Ctrl+Y)" disabled={!canRedo(hist!)} onClick={() => step(redo)}>
              <PhIcon name="arrow-clockwise" size={16} />
            </button>
          </div>          <EditorCanvas
            layout={layout}
            page={page}
            selectedId={selectedId}
            availableWidth={mainWidth - 16}
            onSelect={(id) => { setSelectedId(id); setCreating(null); }}
            onChangePage={setPage}
            hover={session.hover ?? fileHover}
            draggingId={session.dragging?.payload.type === "item" ? session.dragging.payload.id : null}
            preview={swapPreview}
            cellAtRef={cellAtRef}
            onItemPointerDown={session.begin}
            onItemContextMenu={(item, e) => itemMenu(item, e.clientX, e.clientY)}
            onCellContextMenu={(x, y, e, rect) => cellMenu({ x, y }, e.clientX, e.clientY, rect)}
            renamingId={renamingId}
            onRename={(id, label) => {
              setRenamingId(null);
              if (label !== null) patchItem(id, { label: label === "" ? undefined : label });
            }}
            onEmptyCell={(x, y, rect) => { setCreating({ x, y, rect }); setSelectedId(null); setNotice(null); }}
          />
          {page.items.length === 0 && page.popup !== true && (
            <div className="empty-page">
              <strong>This page is empty</strong>
              <div className="row">
                <Button onClick={() => window.dispatchEvent(new CustomEvent("decka:run-setup", { detail: { step: 2 } }))}>Add apps</Button>
                <Button
                  variant="primary"
                  onClick={() => {
                    const cell = document.querySelector(".editor-cell.empty");
                    if (cell) setCreating({ x: 0, y: 0, rect: cell.getBoundingClientRect() });
                  }}
                >
                  Add a button
                </Button>
              </div>
            </div>
          )}
          {notice && !creating && <p className="warn-text">{notice}</p>}

        <div className="form-pane">
          {selected && (
            <ItemForm
              item={selected}
              display={layout.display}
              page={page}
              pages={layout.pages}
              grid={layout.grid}
              actions={actions}
              onChange={(patch) => setPage(updateItem(page, selected.id, patch))}
              onDelete={() => { setPage(removeItem(page, selected.id)); setSelectedId(null); }}
              onChangeAction={() => setCreating({ x: selected.x, y: selected.y, rect: new DOMRect(window.innerWidth / 2 - 180, window.innerHeight / 3, 0, 0), replaceId: selected.id })}
            />
          )}
          {!selected && <p className="muted">Select a key, or click an empty cell to add one.</p>}
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

      {creating && (
        <QuickPicker anchor={creating.rect} cell={creating} page={page} grid={layout.grid} actions={actions} hideWidgets={creating.replaceId !== undefined} onChoose={(c) => void choose(c)} onClose={() => setCreating(null)} />
      )}
      <EditorTips ready={page.items.length > 0 || creating === null} />
      {menu && <ContextMenu x={menu.x} y={menu.y} items={menu.items} onClose={() => setMenu(null)} />}
      {session.dragging && (
        <div className="drag-ghost" style={{ left: session.dragging.x + 12, top: session.dragging.y + 12 }}>
          {ghostLabel(session.dragging.payload)}
        </div>
      )}
      {resizing && (
        <div className="dialog-backdrop">
          <div className="dialog" role="dialog" aria-label="Keys do not fit">
            <h2>
              {resizing.count} key{resizing.count === 1 ? "" : "s"} {resizing.count === 1 ? "doesn't" : "don't"} fit
            </h2>
            <p>Move {resizing.count === 1 ? "it" : "them"} to a new page?</p>
            <div className="row">
              <Button variant="primary" onClick={confirmResize}>Move</Button>
              <Button onClick={() => setResizing(null)}>Cancel</Button>
            </div>
          </div>
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
