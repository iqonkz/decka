import { useCallback, useEffect, useRef, useState } from "react";
import { setPage, unpair } from "../app/controller";
import ConnBadge from "../deck/ConnBadge";
import DimOverlay from "../display/DimOverlay";
import { useDisplayMode } from "../display/idle";
import Grid from "../deck/Grid";
import PageDots from "../deck/PageDots";
import Toasts from "../deck/Toasts";
import { useAppState } from "../store/store";

const SWIPE_MIN_X = 60;
const SWIPE_MAX_Y = 40;
const UNPAIR_TAPS = 5;
const UNPAIR_WINDOW_MS = 3000;
const UNPAIR_ZONE_VMIN = 12;

/**
 * Five taps within 3 s in the top-left corner open the unpair dialog. Counted on `window` in the capture
 * phase by coordinates, so single taps still reach the tiles underneath.
 */
function useUnpairGesture(onTrigger: () => void) {
  useEffect(() => {
    let taps: number[] = [];
    const handler = (e: PointerEvent) => {
      const zone = (Math.min(window.innerWidth, window.innerHeight) * UNPAIR_ZONE_VMIN) / 100;
      if (e.clientX > zone || e.clientY > zone) return;
      const now = Date.now();
      taps = [...taps.filter((t) => now - t < UNPAIR_WINDOW_MS), now];
      if (taps.length >= UNPAIR_TAPS) {
        taps = [];
        onTrigger();
      }
    };
    window.addEventListener("pointerdown", handler, true);
    return () => window.removeEventListener("pointerdown", handler, true);
  }, [onTrigger]);
}

export default function DeckScreen() {
  const layout = useAppState((s) => s.state?.layout ?? null);
  const pageIndex = useAppState((s) => s.pageIndex);
  const start = useRef<{ x: number; y: number } | null>(null);
  const [confirmUnpair, setConfirmUnpair] = useState(false);
  const openUnpair = useCallback(() => setConfirmUnpair(true), []); // stable: the tap counter lives in the effect
  useUnpairGesture(openUnpair);
  const mode = useDisplayMode(layout?.display ?? null);
  if (!layout) return null;

  const count = layout.pages.length;
  const page = layout.pages[pageIndex] ?? layout.pages[0];

  return (
    <>
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
          <ConnBadge />
        </div>
        <Toasts />
      </div>
      <DimOverlay mode={mode} dimLevel={layout.display.dimLevel} />
      {confirmUnpair && (
        <div className="dialog-backdrop">
          <div className="dialog" role="dialog" aria-label="Unpair this device?">
            <h2>Unpair this device?</h2>
            <div className="dialog-actions">
              <button className="secondary" onClick={() => setConfirmUnpair(false)}>
                Cancel
              </button>
              <button
                className="primary danger"
                onClick={() => {
                  setConfirmUnpair(false);
                  unpair();
                }}
              >
                Unpair
              </button>
            </div>
          </div>
        </div>
      )}
    </>
  );
}
