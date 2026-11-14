import type { DisplayMode } from "./idle";

/**
 * Black layer above everything. While dimmed it swallows the next touch (which only wakes the screen,
 * so the button under the finger is not pressed); while active it lets touches through.
 */
export default function DimOverlay({ mode, dimLevel }: { mode: DisplayMode; dimLevel: number }) {
  const dimmed = mode !== "active";
  return (
    <div
      className="dim-overlay"
      style={{ opacity: dimmed ? 1 - dimLevel : 0, pointerEvents: dimmed ? "auto" : "none" }}
      onPointerDown={(e) => e.stopPropagation()}
      onPointerUp={(e) => e.stopPropagation()}
    />
  );
}
