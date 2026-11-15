import { useCallback, useEffect, useState } from "react";
import { setPage, unpair } from "../app/controller";
import ConnBadge from "../deck/ConnBadge";
import DimOverlay from "../display/DimOverlay";
import { useDisplayMode } from "../display/idle";
import Screensaver from "../display/Screensaver";
import { usePixelShift } from "../display/pixelShift";
import { applyTheme, GlassLayer, PageDots, resolveTheme } from "@decka/ui";
import { loadReduced, measureFps, saveReduced, shouldReduce } from "../platform/glassPerf";
import { hapticEvent, isHapticsEnabled, setHapticsEnabled } from "../platform/haptics";
import PhoneMenu from "../menu/PhoneMenu";
import { useMenuGesture } from "../menu/useMenuGesture";
import PageStrip from "../deck/PageStrip";
import PopupPanel from "../deck/PopupPanel";
import { navigableIndices, navigablePosition } from "../deck/popup";
import Toasts from "../deck/Toasts";
import PageHint from "../deck/PageHint";
import { noteTouch, runContext, useAppState } from "../store/store";

const CONTEXT_TICK_MS = 1000;

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
  const [confirmUnpair, setConfirmUnpair] = useState(false);
  const [menuOpen, setMenuOpen] = useState(false);
  const [haptics, setHaptics] = useState(() => isHapticsEnabled());
  const openMenu = useCallback(() => {
    hapticEvent({ type: "menu" });
    setMenuOpen(true);
  }, []);
  // A long press on empty space opens the menu (it replaces the old five taps in the corner).
  const hold = useMenuGesture(openMenu, !menuOpen && !confirmUnpair);
  useContextPages();
  const display = layout?.display;
  const theme = resolveTheme(display);
  const art = useAppState((s) => s.state?.media?.art);
  const popup = useAppState((s) => s.popup);
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

  // Dots and swipe count only normal pages; popup groups are not among them.
  const count = navigableIndices(layout.pages).length;
  const active = navigablePosition(layout.pages, pageIndex);
  const popupPage = popup ? layout.pages.find((p) => p.id === popup.pageId && p.popup === true) : undefined;

  return (
    <>
      {theme.theme === "glass" && <GlassLayer wallpaper={theme.wallpaper} art={art} />}
      <div className={mode === "sleep" ? "deck asleep" : "deck"}>
        <div className="deck-inner" style={{ transform: `translate(${shift.x}px, ${shift.y}px)` }}>
          <PageStrip pages={layout.pages} pageIndex={pageIndex} grid={layout.grid} onCommit={setPage} />
          <PageDots count={count} active={active} />
          <PageHint />
          <ConnBadge />
        </div>
        {popup && popupPage && <PopupPanel page={popupPage} anchor={popup.anchor} grid={layout.grid} />}
        <Toasts />
      </div>
      {hold && (
        <svg className="hold-ring" style={{ left: hold.x - 28, top: hold.y - 28 }} width="56" height="56" viewBox="0 0 56 56" aria-hidden="true">
          <circle cx="28" cy="28" r="24" pathLength="100" />
        </svg>
      )}
      {menuOpen && (
        <PhoneMenu
          reduced={reduced}
          onToggleReduced={(value) => {
            saveReduced(value);
            setReduced(value);
          }}
          haptics={haptics}
          onToggleHaptics={(value) => {
            setHapticsEnabled(value);
            setHaptics(value);
          }}
          onUnpair={() => {
            setMenuOpen(false);
            setConfirmUnpair(true);
          }}
          onClose={() => setMenuOpen(false)}
        />
      )}
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
