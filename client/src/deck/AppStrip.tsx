import { useEffect, useMemo, useRef, useState } from "react";
import { t } from "@decka/shared";
import { gridMetrics, Icon, PopupGrid } from "@decka/ui";
import PhIcon from "../ui/PhIcon";
import { setStripMode, stripModeFor, stripPage, useStripModes, type StripMode } from "./stripMode";
import { useDeckProps } from "./useDeckProps";
import { useAppState } from "../store/store";

const TAP_SLOP_PX = 10;
const TAP_MAX_MS = 400;
const HEADER_PX = 28;

/**
 * The app strip (DESIGN.md 4.10, DATA_MODEL.md 10.5): a panel with the keys of the built-in template of the program in front,
 * at the right edge (portrait: at the bottom).
 */
export default function AppStrip() {
  const strip = useAppState((s) => s.state?.strip);
  const grid = useAppState((s) => s.state?.layout.grid);
  const portrait = useAppState((s) => s.state?.layout.display.orientation === "portrait");
  const modes = useStripModes();
  const props = useDeckProps();
  const [open, setOpen] = useState(false);
  const [menu, setMenu] = useState(false);
  const root = useRef<HTMLDivElement>(null);

  const app = strip?.app ?? null;
  const mode: StripMode = app === null ? "never" : stripModeFor(modes, app);

  // The strip slides out by itself (auto) when its program comes to the front, and rests as a tab otherwise.
  useEffect(() => {
    setOpen(mode === "auto");
    setMenu(false);
  }, [app, mode]);

  // A tap outside folds the strip into its tab and does not reach the key under the finger; a swipe goes through.
  useEffect(() => {
    if (!open || menu) return;
    let down: { x: number; y: number; at: number } | null = null;
    const inside = (e: Event) => e.target instanceof Node && root.current?.contains(e.target) === true;
    const onDown = (e: PointerEvent) => {
      down = inside(e) ? null : { x: e.clientX, y: e.clientY, at: performance.now() };
    };
    const onUp = (e: PointerEvent) => {
      const d = down;
      down = null;
      if (!d || Math.hypot(e.clientX - d.x, e.clientY - d.y) > TAP_SLOP_PX || performance.now() - d.at > TAP_MAX_MS) return;
      e.stopPropagation();
      setOpen(false);
    };
    window.addEventListener("pointerdown", onDown, true);
    window.addEventListener("pointerup", onUp, true);
    return () => {
      window.removeEventListener("pointerdown", onDown, true);
      window.removeEventListener("pointerup", onUp, true);
    };
  }, [open, menu]);

  const page = useMemo(() => (strip ? stripPage(strip, portrait) : null), [strip, portrait]);
  if (!strip || !page || !grid || mode === "never") return null;

  const viewport = { w: window.innerWidth, h: window.innerHeight };
  const { side, gap } = gridMetrics(viewport, grid.cols, grid.rows);
  const pad = Math.round(0.12 * side);
  const across = 2 * side + gap + 2 * pad; // the short side of the strip
  const appIcon = strip.icon ?? "lucide:app-window";
  const edge = portrait ? "strip-bottom" : "strip-right";

  const choose = (next: StripMode) => {
    setStripMode(strip.app, next);
    setMenu(false);
  };

  return (
    <>
      <div
        ref={root}
        className={`app-strip ${edge}${open ? " open" : ""}`}
        style={portrait ? { height: across + HEADER_PX, left: 0, right: 0 } : { width: across }}
        aria-hidden={!open}
      >
        <div className="strip-head" style={{ height: HEADER_PX }}>
          <Icon icon={appIcon} size={16} />
          <span className="strip-name">{strip.name}</span>
          <button type="button" className="strip-more" aria-label={t("strip.options")} onClick={() => setMenu(true)}>
            ⋯
          </button>
        </div>
        <div className="strip-keys" style={{ padding: pad, ["--side" as string]: `${side}px` }}>
          <PopupGrid page={page} side={side} gap={gap} {...props} />
        </div>
      </div>
      {!open && (
        <button type="button" className={`strip-tab ${edge}`} aria-label={t("strip.open", { name: strip.name })} onClick={() => setOpen(true)}>
          <Icon icon={appIcon} size={16} />
        </button>
      )}
      {menu && (
        <div className="menu-backdrop" onPointerDown={() => setMenu(false)}>
          <div className="menu-sheet" role="dialog" aria-label={t("strip.options")} onPointerDown={(e) => e.stopPropagation()}>
            <div className="sheet-handle" aria-hidden="true" />
            <div className="phone-group">
              {(
                [
                  ["auto", t("strip.auto")],
                  ["tab", t("strip.tab_only")],
                  ["never", t("strip.never", { name: strip.name })],
                ] as const
              ).map(([value, label]) => (
                <button key={value} type="button" className="phone-row menu-button" onClick={() => choose(value)}>
                  <PhIcon name="squares-four" size={20} className="phone-row-icon" />
                  <span className="phone-row-label">{label}</span>
                  {mode === value && <PhIcon name="check" size={18} color="var(--accent, #7c5cff)" />}
                </button>
              ))}
            </div>
          </div>
        </div>
      )}
    </>
  );
}
