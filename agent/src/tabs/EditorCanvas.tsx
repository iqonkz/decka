import { useEffect, useMemo, useRef, useState } from "react";
import { t, tn, type InstalledApp, type LayoutFile, type LayoutItem, type Page, type SetPreview, type SysStats, type WeatherInfo } from "@decka/shared";
import { cellRect, deckGeometry, EMPTY_DATA, GlassLayer, Grid, resolveTheme, turnedMisfits, type DeckData } from "@decka/ui";
import { call } from "../ipc";
import { cellFromPoint, type DragPayload } from "./dragMachine";
import { dragSize, itemAt, resizeItem } from "./editorOps";
import { sizeRange, type Grid as GridSize, type Kind } from "./layoutOps";
import PhIcon from "../ui/PhIcon";
import PhoneFrame, { fitScale, phoneView } from "./PhoneFrame";

/** A tall preview never grows beyond this height, so that the editor still fits the window. */
const PORTRAIT_MAX_HEIGHT = 560;
import { useIconPreviews } from "./useIconPreviews";

/** Plausible numbers so that widgets in the preview look alive. */
const SAMPLE_SYS: SysStats = { uptimeSec: 86_400, cpu: 23, mem: { usedMb: 9800, totalMb: 16000 }, disks: [{ mount: "C:", usedGb: 310, totalGb: 476 }], net: { rxKbps: 820, txKbps: 140 } };
/** T32.G: with a volume and a forecast the widgets of the preview have the shape they have on the phone. */
const SAMPLE_VARS = { "audio.volume": 42, "audio.muted": false };
const SAMPLE_WEATHER: WeatherInfo = {
  place: "Almaty",
  units: "metric",
  updatedAt: 0,
  current: { temp: 7, code: 2, isDay: true, wind: 8 },
  daily: [{ date: "2026-11-28", min: 2, max: 9, code: 2 }, { date: "2026-11-29", min: 1, max: 6, code: 3 }, { date: "2026-11-30", min: 0, max: 5, code: 61 }],
};

interface Props {
  layout: LayoutFile;
  page: Page;
  selectedId: string | null;
  availableWidth: number;
  onSelect: (id: string | null) => void;
  /** The page changed by a drag (move, swap, resize). */
  onChangePage: (page: Page) => void;
  /** The cell under the pointer while something is dragged (move, palette or Explorer), highlighted. */
  hover: { x: number; y: number } | null;
  /** While a key hovers over a taken cell: the page as it will be after the drop, so the neighbour glides to the free cell. */
  preview?: Page | null;
  /** A set dragged over the canvas: the cells it would take (a shadow of its shape), and the new page it would get when it does not fit. */
  shadow?: SetPreview | null;
  /** The item being dragged, dimmed. */
  draggingId: string | null;
  /** Filled with a function that maps a screen point to a cell of this canvas, for the drag session. */
  cellAtRef: React.MutableRefObject<(clientX: number, clientY: number) => { x: number; y: number } | null>;
  /** A pointer went down on an item: the start of a click or a drag (pointer events, D-38). */
  onItemPointerDown: (payload: DragPayload, e: React.PointerEvent) => void;
  /** Right click on a key or widget (the window's own menu, DESIGN.md 4.6). */
  onItemContextMenu: (item: LayoutItem, e: React.MouseEvent) => void;
  /** Right click on an empty cell; `rect` is where the cell is on screen. */
  onCellContextMenu: (x: number, y: number, e: React.MouseEvent, rect: DOMRect) => void;
  /** The key whose name is being edited on the key itself (F2 or `Rename`). */
  renamingId: string | null;
  /** The new name (`null` = cancelled). */
  onRename: (id: string, label: string | null) => void;
  /** An empty cell was clicked; `rect` is where it is on screen (the quick picker opens next to it). */
  onEmptyCell: (x: number, y: number, rect: DOMRect) => void;
}

/** The deck as it looks on the phone, in the chosen theme, with drag and drop on top of it (DESIGN.md 4.4). */
export default function EditorCanvas({ layout, page, selectedId, availableWidth, onSelect, onChangePage, hover, draggingId, preview, shadow, cellAtRef, onItemPointerDown, onItemContextMenu, onCellContextMenu, renamingId, onRename, onEmptyCell }: Props) {
  const grid: GridSize = layout.grid;
  // A portrait deck is a tall screen: the preview is kept within an editor-sized height (DESIGN.md 2.2).
  const view = phoneView(layout.display.orientation);
  const scale = fitScale(view, availableWidth, PORTRAIT_MAX_HEIGHT);
  // T32.G: the same geometry as the phone, so that the dotted cells, the drop highlight and the tiles meet to the pixel.
  const m = useMemo(() => deckGeometry(view, grid, layout.display.orientation).metrics, [view, grid, layout.display.orientation]);
  const misfits = layout.display.orientation === "portrait" ? 0 : turnedMisfits(page.items, grid);
  const overlayRef = useRef<HTMLDivElement>(null);
  const resize = useRef<{ id: string; x0: number; y0: number } | null>(null);
  const theme = resolveTheme(layout.display);

  const icons = useIconPreviews(page);

  const { keyFace, keyLabels } = layout.display;
  // The app-keys widget previews the first page that follows an app.
  const appPages = useMemo(() => layout.pages.filter((p) => p.apps !== undefined && p.apps.length > 0), [layout.pages]);
  const previewApp = appPages[0]?.apps?.[0] ?? null;
  const data: DeckData = useMemo(
    () => ({ ...EMPTY_DATA, online: true, preview: true, sys: SAMPLE_SYS, vars: SAMPLE_VARS, weather: SAMPLE_WEATHER, icons, pages: appPages, focusApp: previewApp, ...(keyFace !== undefined ? { keyFace } : {}), ...(keyLabels !== undefined ? { keyLabels } : {}) }),
    [icons, appPages, previewApp, keyFace, keyLabels],
  );
  const noop = useMemo(() => ({ onPress: () => Promise.resolve(), onPressError: () => {}, onNavigate: () => {}, onDismiss: () => {} }), []);

  const rect = (x: number, y: number, w: number, h: number) => {
    const r = cellRect(m, x, y, w, h);
    return { left: r.left * scale, top: r.top * scale, width: r.width * scale, height: r.height * scale };
  };

  cellAtRef.current = (clientX, clientY) => {
    const box = overlayRef.current?.getBoundingClientRect();
    return box ? cellFromPoint(clientX - box.left, clientY - box.top, m, scale, grid) : null;
  };

  const cells: React.ReactNode[] = [];
  for (let y = 0; y < grid.rows; y++) {
    for (let x = 0; x < grid.cols; x++) {
      const taken = itemAt(page, x, y);
      cells.push(
        <div
          key={`c${x}-${y}`}
          className={`editor-cell${taken ? " taken" : " empty"}${hover && hover.x === x && hover.y === y ? (taken ? " over swap" : " over") : ""}`}
          style={rect(x, y, 1, 1)}
          onClick={(e) => (taken ? onSelect(taken.id) : onEmptyCell(x, y, e.currentTarget.getBoundingClientRect()))}
          onContextMenu={(e) => {
            e.preventDefault();
            if (!taken) onCellContextMenu(x, y, e, e.currentTarget.getBoundingClientRect());
          }}
        >
          {!taken && (
            <span className="cell-plus">
              <PhIcon name="plus" size={Math.max(14, Math.round(m.side * scale * 0.3))} />
            </span>
          )}
        </div>,
      );
    }
  }

  const startResize = (e: React.PointerEvent, item: LayoutItem) => {
    e.stopPropagation();
    e.preventDefault();
    (e.currentTarget as HTMLElement).setPointerCapture(e.pointerId);
    resize.current = { id: item.id, x0: e.clientX, y0: e.clientY };
  };
  const endResize = (e: React.PointerEvent, item: LayoutItem) => {
    const r = resize.current;
    resize.current = null;
    if (!r || r.id !== item.id) return;
    const size = dragSize(item, grid, Math.round((e.clientX - r.x0) / ((m.cellW + m.gap) * scale)), Math.round((e.clientY - r.y0) / ((m.cellH + m.gap) * scale)));
    const next = resizeItem(page, grid, item.id, size.w, size.h);
    if (next) onChangePage(next);
  };

  return (
    <div className="editor-canvas-column">
    {misfits > 0 && (
      <div className="editor-misfit" role="status">
        <PhIcon name="warning" size={16} />
        <span>{tn("layout.portrait_misfit", misfits)}</span>
      </div>
    )}
    <div className="editor-canvas">
      <PhoneFrame availableWidth={availableWidth} display={layout.display} maxHeight={PORTRAIT_MAX_HEIGHT}>
        {theme.theme === "glass" && <GlassLayer wallpaper={theme.wallpaper} art={undefined} />}
        <div style={{ position: "absolute", inset: 0 }}>
          <Grid page={preview ?? page} grid={grid} orientation={layout.display.orientation} data={data} animate {...noop} />
        </div>
      </PhoneFrame>
      <div ref={overlayRef} className="editor-overlay" style={{ width: view.w * scale, height: view.h * scale }}>
        {cells}
        {shadow?.cells.map((c, i) => <div key={`s${i}`} className="set-shadow" style={rect(c.x, c.y, c.w, c.h)} />)}
        {shadow?.newPage != null && <div className="set-shadow-hint">{t("layout.set_new_page", { name: shadow.newPage })}</div>}
        {page.items.map((item) => {
          const selected = item.id === selectedId;
          const canResize = selected && (() => {
            const [minW, maxW, minH, maxH] = sizeRange(item.kind, grid);
            return maxW > minW || maxH > minH;
          })();
          return (
            <div
              key={item.id}
              className={`editor-item${selected ? " selected" : ""}${item.id === draggingId ? " dragging" : ""}`}
              style={rect(item.x, item.y, item.w, item.h)}
              onPointerDown={(e) => onItemPointerDown({ type: "item", id: item.id }, e)}
              onContextMenu={(e) => {
                e.preventDefault();
                onItemContextMenu(item, e);
              }}
            >
              {renamingId === item.id && item.kind === "button" && (
                <input
                  className="rename-input"
                  autoFocus
                  aria-label={t("form.name")}
                  defaultValue={item.label ?? ""}
                  maxLength={24}
                  style={{ left: 2, right: 2, bottom: 2 }}
                  onPointerDown={(e) => e.stopPropagation()}
                  onFocus={(e) => e.currentTarget.select()}
                  onBlur={(e) => onRename(item.id, e.currentTarget.value.trim())}
                  onKeyDown={(e) => {
                    if (e.key === "Enter") onRename(item.id, e.currentTarget.value.trim());
                    else if (e.key === "Escape") onRename(item.id, null);
                  }}
                />
              )}
              {canResize && <span className="editor-handle" aria-label={t("canvas.resize")} onPointerDown={(e) => startResize(e, item)} onPointerUp={(e) => endResize(e, item)} />}
            </div>
          );
        })}
      </div>
    </div>
    </div>
  );
}
