import { memo } from "react";
import type { Page } from "@decka/shared";
import { Grid } from "@decka/ui";
import { useDeckProps } from "./useDeckProps";

/**
 * The deck for one page, fed from the store and wired to the controller (the only place where the two meet).
 * Memoized: the strip re-renders on every move of the finger, and the keys must not follow.
 */
function DeckBridge({ page, grid }: { page: Page; grid: { cols: number; rows: number } }) {
  const props = useDeckProps();
  return <Grid page={page} grid={grid} {...props} />;
}

export default memo(DeckBridge);
