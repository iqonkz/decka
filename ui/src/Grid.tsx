import { useEffect, useLayoutEffect, useMemo, useRef, useState } from "react";
import type { ClientLayout, Page } from "@decka/shared";
import { DeckEnvContext, type DeckCallbacks, type DeckData } from "./deckEnv";
import { gridMetrics } from "./gridMetrics";
import { DeckMetricsContext } from "./metricsContext";
import Tile from "./Tile";

export interface GridProps extends DeckCallbacks {
  page: Page;
  grid: ClientLayout["grid"];
  data: DeckData;
  /** The settings window asks for it: a tile that changes cell glides there in 150 ms (a swap in the editor) instead of jumping. */
  animate?: boolean;
}

/** Square keys of equal size with a small gap, centered; empty cells are not drawn. */
export default function Grid({ page, grid, data, animate, onPress, onPressError, onNavigate, onDismiss, onOpenPopup, onFeedback, onTimerDone, onOpenPerm, onOpenPermHistory, onToggleListItem, onLyricsOffset }: GridProps) {
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

  const m = useMemo(() => gridMetrics(viewport, grid.cols, grid.rows), [viewport, grid.cols, grid.rows]);

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
  const value = useMemo(() => ({ side: m.side, gap: m.gap }), [m.side, m.gap]);
  return (
    <div ref={root} className="grid" dir="ltr">
      <DeckEnvContext.Provider value={env}>
      <DeckMetricsContext.Provider value={value}>
        <div
          ref={inner}
          className="grid-inner"
          style={{
            left: m.offsetX,
            top: m.offsetY,
            ["--side" as string]: `${m.side}px`,
            gridTemplateColumns: `repeat(${grid.cols}, ${m.side}px)`,
            gridTemplateRows: `repeat(${grid.rows}, ${m.side}px)`,
            gap: m.gap,
          }}
        >
          {page.items.map((item) => (
            <Tile key={item.id} item={item} />
          ))}
        </div>
      </DeckMetricsContext.Provider>
      </DeckEnvContext.Provider>
    </div>
  );
}
