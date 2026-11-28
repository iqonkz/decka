import { useEffect, useLayoutEffect, useMemo, useRef, useState } from "react";
import type { ClientLayout, Orientation, Page } from "@decka/shared";
import { DeckEnvContext, type DeckCallbacks, type DeckData } from "./deckEnv";
import { deckGeometry, GRID_PAD, GRID_PAD_BOTTOM, transposeItemsForGrid } from "./gridMetrics";
import { DeckMetricsContext } from "./metricsContext";
import Tile from "./Tile";

export interface GridProps extends DeckCallbacks {
  page: Page;
  grid: ClientLayout["grid"];
  data: DeckData;
  /** The settings window asks for it: a tile that changes cell glides there in 150 ms (a swap in the editor) instead of jumping. */
  animate?: boolean;
  /** `display.orientation` of the layout: only a landscape layout is turned on a phone held upright. */
  orientation?: Orientation | undefined;
}

/** Square keys of equal size with a small gap, centered; empty cells are not drawn. */
export default function Grid({ page, grid, data, animate, orientation, onPress, onPressError, onNavigate, onDismiss, onOpenPopup, onFeedback, onTimerDone, onOpenPerm, onOpenPermHistory, onToggleListItem, onLyricsOffset }: GridProps) {
  const env = useMemo(
    () => ({
      ...data,
      onPress,
      onPressError,
      onNavigate,
      onDismiss,
      ...(onOpenPopup ? { onOpenPopup } : {}),
      ...(onFeedback ? { onFeedback } : {}),
      ...(onTimerDone ? { onTimerDone } : {}),
      ...(onOpenPerm ? { onOpenPerm } : {}),
      ...(onOpenPermHistory ? { onOpenPermHistory } : {}),
      ...(onToggleListItem ? { onToggleListItem } : {}),
      ...(onLyricsOffset ? { onLyricsOffset } : {}),
    }),
    [data, onPress, onPressError, onNavigate, onDismiss, onOpenPopup, onFeedback, onTimerDone, onOpenPerm, onOpenPermHistory, onToggleListItem, onLyricsOffset],
  );
  const root = useRef<HTMLDivElement>(null);
  const [viewport, setViewport] = useState({ w: window.innerWidth, h: window.innerHeight });
  useEffect(() => {
    const el = root.current;
    if (!el) return;
    const measure = () => setViewport({ w: el.clientWidth, h: el.clientHeight });
    measure();
    const observer = new ResizeObserver(measure);
    observer.observe(el);
    return () => observer.disconnect();
  }, []);

  const { turned, cols: effCols, rows: effRows, metrics: m } = useMemo(() => deckGeometry(viewport, grid, orientation), [viewport, grid, orientation]);
  const effItems = useMemo(
    () => (turned ? transposeItemsForGrid(page.items, grid.cols, grid.rows, effCols, effRows) : page.items),
    [turned, page.items, grid.cols, grid.rows, effCols, effRows],
  );

  // FLIP: remember where every tile was, and after the next layout slide the ones that moved from the old place to the new one.
  const inner = useRef<HTMLDivElement>(null);
  const seen = useRef(new Map<string, { left: number; top: number }>());
  useLayoutEffect(() => {
    const host = inner.current;
    if (!host || !animate) return;
    const next = new Map<string, { left: number; top: number }>();
    for (const el of Array.from(host.children) as HTMLElement[]) {
      const id = el.dataset.tileId;
      if (!id) continue;
      const box = el.getBoundingClientRect();
      const now = { left: box.left, top: box.top };
      next.set(id, now);
      const before = seen.current.get(id);
      if (before && (Math.abs(before.left - now.left) > 0.5 || Math.abs(before.top - now.top) > 0.5) && typeof el.animate === "function") {
        el.animate([{ transform: `translate(${before.left - now.left}px, ${before.top - now.top}px)` }, { transform: "none" }], { duration: 150, easing: "ease-out" });
      }
    }
    seen.current = next;
  });
  const value = useMemo(() => ({ side: m.side, gap: m.gap, cellW: m.cellW, cellH: m.cellH }), [m.side, m.gap, m.cellW, m.cellH]);
  return (
    <div ref={root} className="grid" dir="ltr">
      <DeckEnvContext.Provider value={env}>
      <DeckMetricsContext.Provider value={value}>
        <div
          ref={inner}
          className="grid-inner"
          style={{
            left: `max(${GRID_PAD}px, env(safe-area-inset-left, 0px))`,
            right: `max(${GRID_PAD}px, env(safe-area-inset-right, 0px))`,
            top: `max(${GRID_PAD}px, env(safe-area-inset-top, 0px))`,
            bottom: `max(${GRID_PAD_BOTTOM}px, calc(${GRID_PAD}px + env(safe-area-inset-bottom, 0px)))`,
            ["--side" as string]: `${m.side}px`,
            gridTemplateColumns: `repeat(${effCols}, minmax(0, 1fr))`,
            gridTemplateRows: `repeat(${effRows}, minmax(0, 1fr))`,
            gap: m.gap,
          }}
        >
          {effItems.map((item) => (
            <Tile key={item.id} item={item} />
          ))}
        </div>
      </DeckMetricsContext.Provider>
      </DeckEnvContext.Provider>
    </div>
  );
}
