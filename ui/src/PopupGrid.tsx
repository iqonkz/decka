import { useMemo } from "react";
import type { Page } from "@decka/shared";
import { DeckEnvContext, type DeckCallbacks, type DeckData } from "./deckEnv";
import { DeckMetricsContext } from "./metricsContext";
import Tile from "./Tile";

/** The bounding box of a popup page's items, in cells: where its first cell is and how many columns and rows it spans. */
export function popupBounds(page: Page): { minX: number; minY: number; cols: number; rows: number } {
  if (page.items.length === 0) return { minX: 0, minY: 0, cols: 1, rows: 1 };
  const minX = Math.min(...page.items.map((i) => i.x));
  const minY = Math.min(...page.items.map((i) => i.y));
  const maxX = Math.max(...page.items.map((i) => i.x + i.w));
  const maxY = Math.max(...page.items.map((i) => i.y + i.h));
  return { minX, minY, cols: maxX - minX, rows: maxY - minY };
}

/** Size in px of the group inside its panel: the same key side and gap as the main grid. */
export function popupSize(page: Page, side: number, gap: number): { width: number; height: number } {
  const { cols, rows } = popupBounds(page);
  return { width: cols * side + (cols - 1) * gap, height: rows * side + (rows - 1) * gap };
}

export interface PopupGridProps extends DeckCallbacks {
  page: Page;
  data: DeckData;
  side: number;
  gap: number;
}

/** The keys of a popup page laid out by their bounding box (DESIGN.md 4.1); the panel around them is the host's. */
export default function PopupGrid({ page, data, side, gap, onPress, onPressError, onNavigate, onDismiss, onOpenPopup, onFeedback, onTimerDone }: PopupGridProps) {
  const env = useMemo(
    () => ({ ...data, onPress, onPressError, onNavigate, onDismiss, ...(onOpenPopup ? { onOpenPopup } : {}), ...(onFeedback ? { onFeedback } : {}), ...(onTimerDone ? { onTimerDone } : {}) }),
    [data, onPress, onPressError, onNavigate, onDismiss, onOpenPopup, onFeedback, onTimerDone],
  );
  const metrics = useMemo(() => ({ side, gap }), [side, gap]);
  const { minX, minY, cols, rows } = popupBounds(page);
  return (
    <DeckEnvContext.Provider value={env}>
      <DeckMetricsContext.Provider value={metrics}>
        <div
          className="popup-grid"
          style={{
            display: "grid",
            gridTemplateColumns: `repeat(${cols}, ${side}px)`,
            gridTemplateRows: `repeat(${rows}, ${side}px)`,
            gap,
            ["--side" as string]: `${side}px`,
          }}
        >
          {page.items.map((item) => (
            <Tile key={item.id} item={{ ...item, x: item.x - minX, y: item.y - minY }} />
          ))}
        </div>
      </DeckMetricsContext.Provider>
    </DeckEnvContext.Provider>
  );
}
