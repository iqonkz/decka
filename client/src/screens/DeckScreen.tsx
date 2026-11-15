import { useCallback, useEffect, useRef, useState } from "react";
import { setPage, unpair } from "../app/controller";
import ConnBadge from "../deck/ConnBadge";
import DimOverlay from "../display/DimOverlay";
import { useDisplayMode } from "../display/idle";
import Screensaver from "../display/Screensaver";
import { usePixelShift } from "../display/pixelShift";
import { applyTheme, GlassLayer, PageDots, resolveTheme } from "@decka/ui";
import { loadReduced, measureFps, saveReduced, shouldReduce } from "../platform/glassPerf";
import DeckBridge from "../deck/DeckBridge";
import Toasts from "../deck/Toasts";
import PageHint from "../deck/PageHint";
import { noteTouch, runContext, useAppState } from "../store/store";

const CONTEXT_TICK_MS = 1000;
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

/**
 * Context pages (DATA_MODEL.md 6.1): remembers the last touch and re-evaluates once a second.
 * Switching a page is not a touch: it never wakes a dimmed screen.
 */
function useContextPages() {
  useEffect(() => {
    const touch = () => noteTouch();
    window.addEventListener("pointerdown", touch, true);
    const timer = window.setInterval(() => runContext(), CONTEXT_TICK_MS);
    return () => {
      window.removeEventListener("pointerdown", touch, true);
      window.clearInterval(timer);
    };
  }, []);
}

export default function DeckScreen() {
  const layout = useAppState((s) => s.state?.layout ?? null);
  const pageIndex = useAppState((s) => s.pageIndex);
  const start = useRef<{ x: number; y: number } | null>(null);
  const [confirmUnpair, setConfirmUnpair] = useState(false);
  const openUnpair = useCallback(() => setConfirmUnpair(true), []); // stable: the tap counter lives in the effect
  useUnpairGesture(openUnpair);
  useContextPages();
  const display = layout?.display;
  const theme = resolveTheme(display);
  const art = useAppState((s) => s.state?.media?.art);
  const [reduced, setReduced] = useState(() => loadReduced() ?? false);
  useEffect(() => {
    applyTheme(document.documentElement, display, { reduced }); // `data-theme` + `--accent` on the root; a missing theme is amoled
  }, [display?.theme, display?.accent, display?.wallpaper, reduced]); // eslint-disable-line react-hooks/exhaustive-deps
  // Glass: measure the frame rate once (3 s); a slow device gets the lighter look and remembers it.
  useEffect(() => {
    if (theme.theme !== "glass" || loadReduced() !== null) return;
    let live = true;
    void measureFps().then((fps) => {
      if (!live) return;
      console.log(`decka glass: ${fps === null ? "unknown" : fps.toFixed(1)} fps`);
      if (fps === null) return;
      const slow = shouldReduce(fps);
      saveReduced(slow);
      setReduced(slow);
    });
    return () => {
      live = false;
    };
  }, [theme.theme]);
  const mode = useDisplayMode(layout?.display ?? null);
  const shift = usePixelShift(layout?.display.pixelShift === true);
  if (!layout) return null;

  const count = layout.pages.length;
  const page = layout.pages[pageIndex] ?? layout.pages[0];

  return (
    <>
      {theme.theme === "glass" && <GlassLayer wallpaper={theme.wallpaper} art={art} />}
      <div
        className={mode === "sleep" ? "deck asleep" : "deck"}
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
        <div className="deck-inner" style={{ transform: `translate(${shift.x}px, ${shift.y}px)` }}>
          {page && <DeckBridge page={page} grid={layout.grid} />}
          <PageDots count={count} active={pageIndex} />
          <PageHint />
          <ConnBadge />
        </div>
        <Toasts />
      </div>
      <DimOverlay mode={mode} dimLevel={layout.display.dimLevel} />
      {mode === "sleep" && <Screensaver />}
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
