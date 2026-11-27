import { createContext, useContext } from "react";

export interface DeckMetrics {
  side: number;
  gap: number;
  cellW?: number;
  cellH?: number;
}

/** Key side, cell dimensions, and gap of the current grid, so keys and panels size themselves from it. */
export const DeckMetricsContext = createContext<DeckMetrics>({ side: 96, gap: 4 });

export function useDeckMetrics(): DeckMetrics {
  return useContext(DeckMetricsContext);
}
