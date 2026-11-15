import { useEffect, useMemo, useRef, useState } from "react";
import type { InstalledApp, LayoutFile, LayoutItem, Page, SysStats } from "@decka/shared";
import { EMPTY_DATA, GlassLayer, Grid, gridMetrics, resolveTheme, type DeckData } from "@decka/ui";
import { call } from "../ipc";
import { dragSize, itemAt, moveItem, resizeItem } from "./editorOps";
import { sizeRange, type Grid as GridSize, type Kind } from "./layoutOps";
import PhoneFrame, { PHONE_H, PHONE_W } from "./PhoneFrame";

/** What can be dragged onto the canvas, as JSON in the drag data. */
export type DragPayload =
  | { type: "item"; id: string }
  | { type: "widget"; kind: Kind }
  | { type: "action"; actionId: string; label: string }
  | { type: "app"; app: InstalledApp };

export const DRAG_MIME = "application/x-decka";

export function readPayload(e: React.DragEvent): DragPayload | null {
  try {
    const raw = e.dataTransfer.getData(DRAG_MIME);
    return raw ? (JSON.parse(raw) as DragPayload) : null;
  } catch {
    return null;
  }
}

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
  /** Something was dropped from the palette on a cell. */
  onDropPayload: (payload: DragPayload, x: number, y: number) => void;
  /** An empty cell was clicked. */
  onEmptyCell: (x: number, y: number) => void;
}

/** The deck as it looks on the phone, in the chosen theme, with drag and drop on top of it (DESIGN.md 4.4). */
export default function EditorCanvas({ layout, page, selectedId, availableWidth, onSelect, onChangePage, onDropPayload, onEmptyCell }: Props) {
  const grid: GridSize = layout.grid;
  const scale = Math.max(0.2, Math.min(1, availableWidth / PHONE_W));
  const m = useMemo(() => gridMetrics({ w: PHONE_W, h: PHONE_H }, grid.cols, grid.rows), [grid.cols, grid.rows]);
  const [hover, setHover] = useState<{ x: number; y: number } | null>(null);
  const [icons, setIcons] = useState<Record<string, string>>({});
  const resize = useRef<{ id: string; x0: number; y0: number } | null>(null);
  const theme = resolveTheme(layout.display);

  // The pictures of `img:` icons used on this page, for the preview.
  const wanted = useMemo(() => {
    const out = new Set<string>();
    for (const item of page.items) {
      if (item.kind !== "button") continue;
      for (const icon of [item.icon, ...Object.values(item.bind?.states ?? {}).map((s) => s.icon)]) if (icon?.startsWith("img:")) out.add(icon);
    }
    return [...out];
  }, [page]);
  useEffect(() => {
    let live = true;
    for (const icon of wanted) {
      if (icons[icon.slice(4)]) continue;
      call("get_icon_preview", { icon }).then(
        (url) => live && url && setIcons((prev) => ({ ...prev, [icon.slice(4)]: url })),
        () => {},
      );
    }
    return () => {
      live = false;
    };
  }, [wanted]); // eslint-disable-line react-hooks/exhaustive-deps

  const data: DeckData = useMemo(() => ({ ...EMPTY_DATA, online: true, sys: SAMPLE_SYS, icons }), [icons]);
  const noop = useMemo(() => ({ onPress: () => Promise.resolve(), onPressError: () => {}, onNavigate: () => {}, onDismiss: () => {} }), []);

  const px = (cell: number, offset: number) => (offset + cell * (m.side + m.gap)) * scale;
  const rect = (x: number, y: number, w: number, h: number) => ({
    left: px(x, m.offsetX),
    top: px(y, m.offsetY),
    width: (w * m.side + (w - 1) * m.gap) * scale,
    height: (h * m.side + (h - 1) * m.gap) * scale,
  });

  const drop = (e: React.DragEvent, x: number, y: number) => {
    e.preventDefault();
    setHover(null);
    const payload = readPayload(e);
    if (!payload) return;
    if (payload.type === "item") {
      const moved = moveItem(page, grid, payload.id, x, y);
      if (moved) onChangePage(moved);
    } else onDropPayload(payload, x, y);
  };
  const over = (e: React.DragEvent, x: number, y: number) => {
    e.preventDefault();
    setHover((h) => (h && h.x === x && h.y === y ? h : { x, y }));
  };

  const cells: React.ReactNode[] = [];
  for (let y = 0; y < grid.rows; y++) {
    for (let x = 0; x < grid.cols; x++) {
      const taken = itemAt(page, x, y);
      cells.push(
        <div
          key={`c${x}-${y}`}
          className={`editor-cell${taken ? " taken" : " empty"}${hover && hover.x === x && hover.y === y ? " over" : ""}`}
          style={rect(x, y, 1, 1)}
          onClick={() => (taken ? onSelect(taken.id) : onEmptyCell(x, y))}
          onDragOver={(e) => over(e, x, y)}
          onDragLeave={() => setHover(null)}
          onDrop={(e) => drop(e, x, y)}
        />,
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
          <Grid page={page} grid={grid} data={data} {...noop} />
        </div>
      </PhoneFrame>
      <div className="editor-overlay" style={{ width: PHONE_W * scale, height: PHONE_H * scale }}>
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
              className={`editor-item${selected ? " selected" : ""}`}
              style={rect(item.x, item.y, item.w, item.h)}
              draggable
              onDragOver={(e) => over(e, item.x, item.y)}
              onDragLeave={() => setHover(null)}
              onDrop={(e) => drop(e, item.x, item.y)}
              onDragStart={(e) => {
                e.dataTransfer.setData(DRAG_MIME, JSON.stringify({ type: "item", id: item.id } satisfies DragPayload));
                e.dataTransfer.effectAllowed = "move";
              }}
              onClick={() => onSelect(item.id)}
            >
              {canResize && <span className="editor-handle" aria-label="Resize" onPointerDown={(e) => startResize(e, item)} onPointerUp={(e) => endResize(e, item)} />}
            </div>
          );
        })}
      </div>
    </div>
  );
}
