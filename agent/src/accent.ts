/** The accent of a deck that never chose one (`display.accent`). */
const DEFAULT_ACCENT = "#7c5cff";

/**
 * The accent of the deck is also the accent of the window. The default purple is left to the stylesheet, which has a shade
 * of it for each window theme with readable contrast (DESIGN.md 8.1); an accent the user chose is used as it is.
 */
export function applyAccent(accent: string | undefined, root: HTMLElement = document.documentElement): void {
  if (accent === undefined || accent.toLowerCase() === DEFAULT_ACCENT) root.style.removeProperty("--accent-chosen");
  else root.style.setProperty("--accent-chosen", accent);
}
