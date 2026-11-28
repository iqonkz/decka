import type { InstalledApp } from "@decka/shared";
import type { Kind } from "./layoutOps";

/** What can be dragged onto the canvas (DESIGN.md 4.4, 4.6). Pointer events only: HTML5 drag and drop does not work in the Tauri window (D-38). */
export type DragPayload =
  | { type: "item"; id: string }
  | { type: "widget"; kind: Kind }
  | { type: "action"; actionId: string; label: string }
  | { type: "app"; app: InstalledApp }
  | { type: "catalog"; itemId: string; label: string }
  | { type: "set"; setId: string; label: string };

/** Pixels the pointer must travel before a press becomes a drag (DESIGN.md 4.6). */
export const DRAG_THRESHOLD = 4;

export type DragState =
  | { phase: "idle" }
  | { phase: "pressed"; payload: DragPayload; x0: number; y0: number }
  | { phase: "dragging"; payload: DragPayload; x0: number; y0: number; x: number; y: number };

export type DragEvent =
  | { type: "down"; payload: DragPayload; x: number; y: number }
  | { type: "move"; x: number; y: number }
  | { type: "up"; x: number; y: number }
  | { type: "cancel" };

/** What a finished gesture asks for: a click (no movement), a drop at a pointer position, or nothing (cancelled). */
export type DragResult =
  | { kind: "click"; payload: DragPayload }
  | { kind: "drop"; payload: DragPayload; x: number; y: number }
  | { kind: "cancel"; payload: DragPayload };

export const IDLE: DragState = { phase: "idle" };

/**
 * The state machine of one drag: idle -> pressed -> dragging -> drop / cancel.
 * A press that ends before the pointer moved `DRAG_THRESHOLD` px is a click; `cancel` (Esc, lost capture) drops the gesture.
 */
export function dragReducer(state: DragState, event: DragEvent): { state: DragState; result?: DragResult } {
  switch (state.phase) {
    case "idle":
      return event.type === "down" ? { state: { phase: "pressed", payload: event.payload, x0: event.x, y0: event.y } } : { state };
    case "pressed":
      if (event.type === "move") {
        const far = Math.hypot(event.x - state.x0, event.y - state.y0) >= DRAG_THRESHOLD;
        return far ? { state: { phase: "dragging", payload: state.payload, x0: state.x0, y0: state.y0, x: event.x, y: event.y } } : { state };
      }
      if (event.type === "up") return { state: IDLE, result: { kind: "click", payload: state.payload } };
      if (event.type === "cancel") return { state: IDLE, result: { kind: "cancel", payload: state.payload } };
      return { state };
    case "dragging":
      if (event.type === "move") return { state: { ...state, x: event.x, y: event.y } };
      if (event.type === "up") return { state: IDLE, result: { kind: "drop", payload: state.payload, x: event.x, y: event.y } };
      if (event.type === "cancel") return { state: IDLE, result: { kind: "cancel", payload: state.payload } };
      return { state };
  }
}

/**
 * The grid cell under a point of the overlay, or `null` outside the grid. `px`/`py` are measured from the overlay's
 * top-left corner in screen pixels; `scale` is the preview scale, the rest is `gridMetrics` of the phone (without `cellW`
 * and `cellH` the cells are squares of `side`).
 */
export function cellFromPoint(
  px: number,
  py: number,
  m: { offsetX: number; offsetY: number; side: number; gap: number; cellW?: number; cellH?: number },
  scale: number,
  grid: { cols: number; rows: number },
): { x: number; y: number } | null {
  const fx = (px / scale - m.offsetX) / ((m.cellW ?? m.side) + m.gap);
  const fy = (py / scale - m.offsetY) / ((m.cellH ?? m.side) + m.gap);
  const x = Math.floor(fx);
  const y = Math.floor(fy);
  if (x < 0 || y < 0 || x >= grid.cols || y >= grid.rows) return null;
  return { x, y };
}
