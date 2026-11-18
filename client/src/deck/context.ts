import type { DisplaySettings, FocusInfo, Page } from "@decka/shared";

/** A touch this recent keeps the page where it is (the user is working with the phone). */
export const TOUCH_QUIET_MS = 5000;

export interface ContextInput {
  pages: Page[];
  display: Pick<DisplaySettings, "contextPages">;
  focusApp: FocusInfo | null | undefined;
  pageIndex: number;
  homePageIndex: number;
  autoPageId: string | null;
  now: number;
  lastTouch: number;
}

export interface ContextDecision {
  pageIndex: number;
  autoPageId: string | null;
  /** Name of the page that was just opened automatically; null = no hint. */
  hint: string | null;
}

/** Whether the time of day of `now` (the phone's clock) is inside `from`..`to`; `from` later than `to` crosses midnight. */
export function inTimeRange(time: { from: string; to: string }, now: number): boolean {
  const minutes = (s: string) => Number(s.slice(0, 2)) * 60 + Number(s.slice(3, 5));
  const d = new Date(now);
  const at = d.getHours() * 60 + d.getMinutes();
  const [from, to] = [minutes(time.from), minutes(time.to)];
  return from < to ? at >= from && at < to : at >= from || at < to;
}

/** DATA_MODEL.md 6.1: which page to show for the active PC application. */
export function decideContextPage(input: ContextInput): ContextDecision {
  const { pages, display, focusApp, pageIndex, homePageIndex, autoPageId, now, lastTouch } = input;
  const unchanged: ContextDecision = { pageIndex, autoPageId, hint: null };
  if (display.contextPages === false) return unchanged; // 1
  if (now - lastTouch < TOUCH_QUIET_MS) return unchanged; // 2

  const app = focusApp?.app ?? null;
  const byApp = app === null ? -1 : pages.findIndex((p) => p.apps?.includes(app) === true);
  // A page with hours of the day follows the clock when no program page applies.
  const found = byApp >= 0 ? byApp : pages.findIndex((p, i) => i > 0 && p.popup !== true && p.time !== undefined && inTimeRange(p.time, now));
  const target = found >= 0 ? found : null; // 3

  if (target !== null && target !== pageIndex) {
    return { pageIndex: target, autoPageId: pages[target]!.id, hint: pages[target]!.name }; // 4
  }
  if (target === null && autoPageId !== null) {
    const autoIndex = pages.findIndex((p) => p.id === autoPageId);
    // Back home when the user is still on the automatic page, or when that page no longer exists.
    if (autoIndex < 0 || autoIndex === pageIndex) {
      return { pageIndex: homePageIndex < pages.length ? homePageIndex : 0, autoPageId: null, hint: null }; // 5
    }
  }
  return unchanged; // 6
}
