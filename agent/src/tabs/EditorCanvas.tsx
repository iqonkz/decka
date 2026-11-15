import { useEffect, useMemo, useRef, useState } from "react";
import { t, type InstalledApp, type LayoutFile, type LayoutItem, type Page, type SysStats } from "@decka/shared";
import { EMPTY_DATA, GlassLayer, Grid, gridMetrics, resolveTheme, type DeckData } from "@decka/ui";
import { call } from "../ipc";
import { cellFromPoint, type DragPayload } from "./dragMachine";
import { dragSize, itemAt, resizeItem } from "./editorOps";
import { sizeRange, type Grid as GridSize, type Kind } from "./layoutOps";
import PhIcon from "../ui/PhIcon";
import PhoneFrame, { PHONE_H, PHONE_W } from "./PhoneFrame";
import { useIconPreviews } from "./useIconPreviews";

/** Plausible numbers so that widgets in the preview look alive. */
const SAMPLE_SYS: SysStats = { uptimeSec: 86_400, cpu: 23, mem: { usedMb: 9800, totalMb: 16000 }, disks: [{ mount: "C:", usedGb: 310, totalGb: 476 }], net: { rxKbps: 820, txKbps: 140 } };

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
export default function EditorCanvas({ layout, page, selectedId, availableWidth, onSelect, onChangePage, hover, draggingId, preview, cellAtRef, onItemPointerDown, onItemContextMenu, onCellContextMenu, renamingId, onRename, onEmptyCell }: Props) {
  const grid: GridSize = layout.grid;
  const scale = Math.max(0.2, Math.min(1, availableWidth / PHONE_W));
  const m = useMemo(() => gridMetrics({ w: PHONE_W, h: PHONE_H }, grid.cols, grid.rows), [grid.cols, grid.rows]);
  const overlayRef = useRef<HTMLDivElement>(null);
  const resize = useRef<{ id: string; x0: number; y0: number } | null>(null);
  const theme = resolveTheme(layout.display);

  const icons = useIconPreviews(page);

  const { keyFace, keyLabels } = layout.display;
  const data: DeckData = useMemo(
    () => ({ ...EMPTY_DATA, online: true, sys: SAMPLE_SYS, icons, ...(keyFace !== undefined ? { keyFace } : {}), ...(keyLabels !== undefined ? { keyLabels } : {}) }),
    [icons, keyFace, keyLabels],
  );
  const noop = useMemo(() => ({ onPress: () => Promise.resolve(), onPressError: () => {}, onNavigate: () => {}, onDismiss: () => {} }), []);

  const px = (cell: number, offset: number) => (offset + cell * (m.side + m.gap)) * scale;
  const rect = (x: number, y: number, w: number, h: number) => ({
    left: px(x, m.offsetX),
    top: px(y, m.offsetY),
    width: (w * m.side + (w - 1) * m.gap) * scale,
    height: (h * m.side + (h - 1) * m.gap) * scale,
  });

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
    const cell = (m.side + m.gap) * scale;
    const size = dragSize(item, grid, Math.round((e.clientX - r.x0) / cell), Math.round((e.clientY - r.y0) / cell));
    const next = resizeItem(page, grid, item.id, size.w, size.h);
    if (next) onChangePage(next);
  };

  return (
    <div className="editor-canvas">
      <PhoneFrame availableWidth={availableWidth} display={layout.display}>
        {theme.theme === "glass" && <GlassLayer wallpaper={theme.wallpaper} art={undefined} />}
        <div style={{ position: "absolute", inset: 0 }}>
          <Grid page={preview ?? page} grid={grid} data={data} animate {...noop} />
        </div>
      </PhoneFrame>
      <div ref={overlayRef} className="editor-overlay" style={{ width: PHONE_W * scale, height: PHONE_H * scale }}>
        {cells}
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
  );
}
