/** Brand logos (simple-icons, CC0): `public/brands.json` rows are `[title, slug, hex]`, made at build time. */
export interface Brand {
  title: string;
  slug: string;
  hex: string;
}

/** Shown first when the search box is empty. */
const POPULAR = ["discord", "steam", "spotify", "obsstudio", "twitch", "youtube", "telegram", "whatsapp", "github", "googlechrome", "firefox", "microsoftedge", "visualstudiocode", "notion", "slack", "zoom", "netflix", "epicgames", "battledotnet", "nvidia", "amd", "intel", "x", "instagram", "tiktok", "reddit", "vlc", "figma", "gmail", "googledrive"];

let cache: Promise<Brand[]> | null = null;
export function loadBrands(): Promise<Brand[]> {
  cache ??= fetch("brands.json")
    .then((r) => r.json() as Promise<[string, string, string][]>)
    .then((rows) => rows.map(([title, slug, hex]) => ({ title, slug, hex })));
  return cache;
}

export const brandUrl = (slug: string): string => `brands/${slug}.svg`;

/** Brands whose title contains every word of `query`; titles that start with the first word come first. An empty query lists the popular ones, then the rest. */
export function searchBrands(list: readonly Brand[], query: string): Brand[] {
  const words = query.toLowerCase().split(/\s+/).filter(Boolean);
  if (words.length === 0) {
    const rank = (b: Brand) => {
      const i = POPULAR.indexOf(b.slug);
      return i < 0 ? POPULAR.length : i;
    };
    return [...list].sort((a, b) => rank(a) - rank(b));
  }
  const scored: [number, Brand][] = [];
  for (const b of list) {
    const title = `${b.title} ${b.slug}`.toLowerCase();
    if (!words.every((w) => title.includes(w))) continue;
    scored.push([b.title.toLowerCase().startsWith(words[0] as string) ? 0 : 1, b]);
  }
  return scored.sort((a, b) => a[0] - b[0] || a[1].title.localeCompare(b[1].title)).map(([, b]) => b);
}
