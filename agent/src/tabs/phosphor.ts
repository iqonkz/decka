import { icons } from "@phosphor-icons/core";

export const WEIGHTS = ["regular", "thin", "light", "bold", "fill", "duotone"] as const;
export type Weight = (typeof WEIGHTS)[number];

export interface PhosphorEntry {
  name: string;
  tags: readonly string[];
}

/** The icon index, built from the package when the settings window is built. */
export const PHOSPHOR_ICONS: readonly PhosphorEntry[] = icons.map((i) => ({ name: i.name, tags: i.tags.filter((t) => !t.startsWith("*")) }));

/** File name of an icon in a weight: `heart.svg` for regular, `heart-bold.svg` for the others. */
export function svgFileName(name: string, weight: Weight): string {
  return weight === "regular" ? `${name}.svg` : `${name}-${weight}.svg`;
}

/** Where the settings window finds the SVG of an icon (a static copy made at build time). */
export function svgUrl(name: string, weight: Weight): string {
  return `phosphor/${weight}/${svgFileName(name, weight)}`;
}

/**
 * Icons whose name or tags contain every word of `query` (ignoring case); names that start with the first word come
 * first, then other name matches, then tag matches. An empty query lists everything in name order.
 */
export function searchIcons(query: string, list: readonly PhosphorEntry[] = PHOSPHOR_ICONS): PhosphorEntry[] {
  const words = query.toLowerCase().split(/\s+/).filter(Boolean);
  if (words.length === 0) return [...list];
  const scored: [number, PhosphorEntry][] = [];
  for (const entry of list) {
    const name = entry.name.toLowerCase();
    const tags = entry.tags.map((t) => t.toLowerCase());
    const hit = (w: string) => name.includes(w) || tags.some((t) => t.includes(w));
    if (!words.every(hit)) continue;
    const first = words[0] as string;
    const score = name.startsWith(first) ? 0 : name.includes(first) ? 1 : 2;
    scored.push([score, entry]);
  }
  return scored.sort((a, b) => a[0] - b[0] || a[1].name.localeCompare(b[1].name)).map(([, e]) => e);
}

/** The rows of a grid that are in view (plus `overscan` rows around) as `[first, end)`; nothing outside is rendered. */
export function visibleRows(scrollTop: number, viewportHeight: number, rowHeight: number, rowCount: number, overscan = 2): [number, number] {
  if (rowCount <= 0 || rowHeight <= 0) return [0, 0];
  const top = Math.max(0, Math.min(scrollTop, rowCount * rowHeight - viewportHeight)); // a scroll position past the end is clamped
  const first = Math.max(0, Math.floor(top / rowHeight) - overscan);
  const end = Math.min(rowCount, Math.ceil((top + viewportHeight) / rowHeight) + overscan);
  return [first, Math.max(first, end)];
}
