import type { Page } from "@decka/shared";

export interface Rect {
  x: number;
  y: number;
  width: number;
  height: number;
}

/** Space between the key and its panel. */
export const POPUP_GAP_PX = 8;
/** Seconds without a touch after which the panel closes (DESIGN.md 4.1). */
export const POPUP_IDLE_MS = 10_000;
/** Delay between a successful press inside the panel and its closing. */
export const POPUP_CLOSE_DELAY_MS = 250;
/** A downward swipe on the panel of at least this many px closes it. */
export const POPUP_SWIPE_DOWN_PX = 48;

/**
 * Where the panel goes: centred above the pressed key (below it when there is no room above), shifted so that it
 * lies completely on the screen with `pad` px to spare. A panel larger than the screen is pinned to the padding.
 */
export function placePopup(anchor: Rect, panel: { width: number; height: number }, viewport: { width: number; height: number }, pad: number): { left: number; top: number } {
  let left = anchor.x + anchor.width / 2 - panel.width / 2;
  let top = anchor.y - panel.height - POPUP_GAP_PX;
  if (top < pad) top = anchor.y + anchor.height + POPUP_GAP_PX;
  const clamp = (v: number, size: number, room: number) => Math.max(pad, Math.min(v, room - size - pad));
  left = clamp(left, panel.width, viewport.width);
  top = clamp(top, panel.height, viewport.height);
  return { left: Math.round(left), top: Math.round(top) };
}

/** Indices of the pages the user can swipe between: popup groups are not among them. */
export function navigableIndices(pages: Page[]): number[] {
  return pages.flatMap((p, i) => (p.popup === true ? [] : [i]));
}

/** Position of page `index` among the navigable pages (0 for a popup page or an unknown index). */
export function navigablePosition(pages: Page[], index: number): number {
  return Math.max(0, navigableIndices(pages).indexOf(index));
}
