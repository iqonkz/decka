import { useCallback, useEffect, useRef, useState } from "react";
import { dragReducer, IDLE, type DragEvent, type DragPayload, type DragResult, type DragState } from "./dragMachine";

export interface Cell {
  x: number;
  y: number;
}

interface Options {
  /** Maps a screen point to a cell of the canvas. */
  cellAt: (clientX: number, clientY: number) => Cell | null;
  /** The drag ended on a cell. */
  onDrop: (payload: DragPayload, cell: Cell) => void;
  /** A press ended without a drag. */
  onClick: (payload: DragPayload) => void;
}

/**
 * Drag and drop on pointer events (D-38; HTML5 drag and drop does not work in the Tauri window). `begin` goes on the
 * `onPointerDown` of whatever can be dragged; the session listens on `window` until the pointer is released or `Esc` is pressed.
 */
export function useDragSession(options: Options) {
  const [state, setState] = useState<DragState>(IDLE);
  const stateRef = useRef<DragState>(IDLE);
  const opts = useRef(options);
  opts.current = options;

  const apply = useCallback((event: DragEvent) => {
    const out = dragReducer(stateRef.current, event);
    stateRef.current = out.state;
    setState(out.state);
    const result: DragResult | undefined = out.result;
    if (!result) return;
    if (result.kind === "click") opts.current.onClick(result.payload);
    else if (result.kind === "drop") {
      const cell = opts.current.cellAt(result.x, result.y);
      if (cell) opts.current.onDrop(result.payload, cell);
    }
  }, []);

  const begin = useCallback(
    (payload: DragPayload, e: React.PointerEvent) => {
      if (e.button !== 0 || stateRef.current.phase !== "idle") return;
      e.preventDefault(); // no text selection while dragging
      try {
        (e.currentTarget as HTMLElement).setPointerCapture(e.pointerId);
      } catch {
        // the element is gone or capture is not available: window listeners still get the events
      }
      apply({ type: "down", payload, x: e.clientX, y: e.clientY });
    },
    [apply],
  );

  const active = state.phase !== "idle";
  useEffect(() => {
    if (!active) return;
    const move = (e: PointerEvent) => apply({ type: "move", x: e.clientX, y: e.clientY });
    const up = (e: PointerEvent) => apply({ type: "up", x: e.clientX, y: e.clientY });
    const cancel = () => apply({ type: "cancel" });
    const key = (e: KeyboardEvent) => {
      if (e.key === "Escape") cancel();
    };
    window.addEventListener("pointermove", move);
    window.addEventListener("pointerup", up);
    window.addEventListener("pointercancel", cancel);
    window.addEventListener("keydown", key);
    return () => {
      window.removeEventListener("pointermove", move);
      window.removeEventListener("pointerup", up);
      window.removeEventListener("pointercancel", cancel);
      window.removeEventListener("keydown", key);
    };
  }, [active, apply]);

  const dragging = state.phase === "dragging" ? state : null;
  const hover = dragging ? opts.current.cellAt(dragging.x, dragging.y) : null;
  return { begin, dragging, hover };
}