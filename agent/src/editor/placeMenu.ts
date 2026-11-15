/**
 * Where a context menu of `size` opens for a click at `point` in a `viewport`: at the point, moved inside the window when it
 * would stick out (DESIGN.md 4.6). A menu larger than the window sits at the margin.
 */
export function placeMenu(point: { x: number; y: number }, size: { w: number; h: number }, viewport: { w: number; h: number }, margin = 4): { left: number; top: number } {
  const clamp = (v: number, max: number) => Math.max(margin, Math.min(v, max));
  return { left: clamp(point.x, viewport.w - size.w - margin), top: clamp(point.y, viewport.h - size.h - margin) };
}