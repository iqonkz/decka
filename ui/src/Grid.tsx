import { useEffect, useMemo, useRef, useState } from "react";
import type { ClientLayout, Page } from "@decka/shared";
import { DeckEnvContext, type DeckCallbacks, type DeckData } from "./deckEnv";
import { gridMetrics } from "./gridMetrics";
import { DeckMetricsContext } from "./metricsContext";
import Tile from "./Tile";

export interface GridProps extends DeckCallbacks {
  page: Page;
  grid: ClientLayout["grid"];
  data: DeckData;
}

/** Square keys of equal size with a small gap, centered; empty cells are not drawn. */
export default function Grid({ page, grid, data, onPress, onPressError, onNavigate, onDismiss, onOpenPopup, onFeedback }: GridProps) {
  const env = useMemo(
    () => ({ ...data, onPress, onPressError, onNavigate, onDismiss, ...(onOpenPopup ? { onOpenPopup } : {}), ...(onFeedback ? { onFeedback } : {}) }),
    [data, onPress, onPressError, onNavigate, onDismiss, onOpenPopup, onFeedback],
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
  const value = useMemo(() => ({ side: m.side, gap: m.gap }), [m.side, m.gap]);
  return (
    <div ref={root} className="grid">
      <DeckEnvContext.Provider value={env}>
      <DeckMetricsContext.Provider value={value}>
        <div
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
