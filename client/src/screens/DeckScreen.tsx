import { useRef } from "react";
import { setPage } from "../app/controller";
import Grid from "../deck/Grid";
import PageDots from "../deck/PageDots";
import { useAppState } from "../store/store";

const SWIPE_MIN_X = 60;
const SWIPE_MAX_Y = 40;

export default function DeckScreen() {
  const layout = useAppState((s) => s.state?.layout ?? null);
  const pageIndex = useAppState((s) => s.pageIndex);
  const start = useRef<{ x: number; y: number } | null>(null);
  if (!layout) return null;

  const count = layout.pages.length;
  const page = layout.pages[pageIndex] ?? layout.pages[0];

  return (
    <div
      className="deck"
      onPointerDown={(e) => {
        start.current = { x: e.clientX, y: e.clientY };
      }}
      onPointerUp={(e) => {
        const s = start.current;
        start.current = null;
        if (!s) return;
        const dx = e.clientX - s.x;
        const dy = Math.abs(e.clientY - s.y);
        if (Math.abs(dx) >= SWIPE_MIN_X && dy < SWIPE_MAX_Y) {
          // No wrap-around, no animation.
          const next = dx < 0 ? Math.min(pageIndex + 1, count - 1) : Math.max(pageIndex - 1, 0);
          if (next !== pageIndex) setPage(next);
        }
      }}
      onPointerCancel={() => {
        start.current = null;
      }}
    >
      <div className="deck-inner">
        {page && <Grid page={page} />}
        <PageDots count={count} active={pageIndex} />
      </div>
    </div>
  );
}
