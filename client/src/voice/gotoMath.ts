/** Pure parts of "go to page X" by voice (see goto.ts). */

/** The page id of a value of `voice.goto`; `null` for anything that is not `<id>#<counter>`. */
export function parseGoto(value: unknown): string | null {
  if (typeof value !== "string") return null;
  const m = /^([a-z0-9_-]{1,64})#\d{1,12}$/.exec(value);
  return m ? m[1]! : null;
}

/** Index of the page in the layout, popups excluded; `-1` when there is none. */
export function pageIndexOf(pages: { id: string; popup?: boolean }[], id: string): number {
  return pages.findIndex((p) => p.id === id && p.popup !== true);
}
