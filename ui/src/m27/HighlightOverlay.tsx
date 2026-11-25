import { t } from "@decka/shared";
import type { HighlightEvent, HighlightRect } from "./m27Types";
import "./m27.css";

export type BubbleSide = "bottom" | "top" | "right" | "left";

export interface BubblePlacement {
  side: BubbleSide;
  left: number;
  top: number;
  arrowOffset: number;
}

const GAP = 14;
const EDGE = 8;

/**
 * Picks an automatic side around `rect` (preferring bottom -> top -> right -> left) so the <=280px bubble stays inside the viewport.
 */
export function computeBubblePlacement(
  rect: HighlightRect,
  viewport: { width: number; height: number },
  bubbleSize: { width: number; height: number } = { width: 280, height: 84 },
): BubblePlacement {
  const bw = Math.min(280, Math.max(120, bubbleSize.width));
  const bh = Math.max(48, bubbleSize.height);
  const clamp = (v: number, min: number, max: number) => Math.max(min, Math.min(max, v));
  const cx = rect.x + rect.width / 2;
  const cy = rect.y + rect.height / 2;

  const fitsBottom = rect.y + rect.height + GAP + bh + EDGE <= viewport.height;
  const fitsTop = rect.y - GAP - bh - EDGE >= 0;
  const fitsRight = rect.x + rect.width + GAP + bw + EDGE <= viewport.width;
  const fitsLeft = rect.x - GAP - bw - EDGE >= 0;

  let side: BubbleSide = "bottom";
  if (fitsBottom) side = "bottom";
  else if (fitsTop) side = "top";
  else if (fitsRight) side = "right";
  else if (fitsLeft) side = "left";
  else side = cy < viewport.height / 2 ? "bottom" : "top";

  if (side === "bottom" || side === "top") {
    const left = clamp(cx - bw / 2, EDGE, Math.max(EDGE, viewport.width - bw - EDGE));
    const rawTop = side === "bottom" ? rect.y + rect.height + GAP : rect.y - GAP - bh;
    const top = clamp(rawTop, EDGE, Math.max(EDGE, viewport.height - bh - EDGE));
    const arrowOffset = clamp(cx - left, 16, bw - 16);
    return { side, left: Math.round(left), top: Math.round(top), arrowOffset: Math.round(arrowOffset) };
  }

  const left = clamp(
    side === "right" ? rect.x + rect.width + GAP : rect.x - GAP - bw,
    EDGE,
    Math.max(EDGE, viewport.width - bw - EDGE),
  );
  const top = clamp(cy - bh / 2, EDGE, Math.max(EDGE, viewport.height - bh - EDGE));
  const arrowOffset = clamp(cy - top, 14, bh - 14);
  return { side, left: Math.round(left), top: Math.round(top), arrowOffset: Math.round(arrowOffset) };
}

export interface HighlightOverlayProps {
  highlight: HighlightEvent | null;
  theme?: "light" | "dark";
  accent?: string;
  viewport?: { width: number; height: number };
  reducedMotion?: boolean;
  /** Render inside a relative container (demo/screenshot) instead of fixed fullscreen overlay. */
  contained?: boolean;
}

/**
 * Click-through highlight frame (`2px` accent, `12px` glow, radius `8`, `1.2s` pulse) + auto-placed glass bubble (`<= 280px`).
 */
export default function HighlightOverlay({
  highlight,
  theme = "dark",
  accent = "#7c5cff",
  viewport,
  reducedMotion = false,
  contained = false,
}: HighlightOverlayProps) {
  if (!highlight) return null;
  const vp = viewport ?? {
    width: typeof window !== "undefined" ? window.innerWidth : 1280,
    height: typeof window !== "undefined" ? window.innerHeight : 720,
  };
  const { rect, text, step, total } = highlight;
  const hasStep = typeof step === "number" && step > 0;
  const estWidth = Math.min(280, Math.max(120, Math.max(text.length, hasStep ? 14 : 0) * 8 + 32));
  const estHeight = hasStep ? 86 : 64;
  const placement = computeBubblePlacement(rect, vp, { width: estWidth, height: estHeight });

  return (
    <div
      className={`m27-highlight-layer${contained ? " is-contained" : ""}${reducedMotion ? " is-reduced" : ""}`}
      data-m27-theme={theme}
      style={{ "--m27-accent": accent } as React.CSSProperties}
      aria-live="polite"
    >
      <div
        className="m27-highlight-frame"
        style={{
          left: rect.x,
          top: rect.y,
          width: rect.width,
          height: rect.height,
        }}
      />
      <div
        className={`m27-bubble m27-bubble--${placement.side}`}
        role="status"
        style={{
          left: placement.left,
          top: placement.top,
          "--m27-arrow": `${placement.arrowOffset}px`,
        } as React.CSSProperties}
      >
        {hasStep && (
          <div className="m27-step-badge">
            {total && total > 0 ? t("m27.step_of", { n: step, total }) : t("m27.step", { n: step })}
          </div>
        )}
        <div className="m27-bubble-text">{text}</div>
      </div>
    </div>
  );
}
