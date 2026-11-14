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

/** DATA_MODEL.md 6.1: which page to show for the active PC application. */
export function decideContextPage(input: ContextInput): ContextDecision {
  const { pages, display, focusApp, pageIndex, homePageIndex, autoPageId, now, lastTouch } = input;
  const unchanged: ContextDecision = { pageIndex, autoPageId, hint: null };
  if (display.contextPages === false) return unchanged; // 1
  if (now - lastTouch < TOUCH_QUIET_MS) return unchanged; // 2

  const app = focusApp?.app ?? null;
  const found = app === null ? -1 : pages.findIndex((p) => p.apps?.includes(app) === true);
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
