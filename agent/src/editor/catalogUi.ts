import { CATALOG, catalogItem, getLanguage, type CatalogItem, type CatalogSet, type InsertContext, type LayoutFile } from "@decka/shared";
import { call } from "../ipc";
import { svgUrl } from "../tabs/phosphor";

/** What the library knows about the integrations: the ones that are on get no `Set up` mark. */
export interface Integrations { obs: boolean; weather: boolean; twitch: boolean; }

export async function loadIntegrations(): Promise<Integrations> {
  const config = await call("get_config").catch(() => null);
  return { obs: config?.obs?.enabled === true, weather: config?.weather?.enabled === true, twitch: config?.twitch?.enabled === true && config.twitch.tokensSet === true };
}

/** The OBS scenes and audio inputs, or `null` when OBS is not connected (or this agent has no OBS yet). */
export async function loadObs(): Promise<{ scenes: string[]; inputs: string[] } | null> {
  const status = await call("obs_status").catch(() => null);
  return status && status.connected ? { scenes: status.scenes, inputs: status.inputs } : null;
}

const iconCache = new Map<string, string>();

/** `ph:shuffle` -> `img:<hash16>`: the SVG of the icon goes through the agent (`icon_from_svg`, DATA_MODEL.md 8.4). */
async function phosphorIcon(icon: string): Promise<string | null> {
  const hit = iconCache.get(icon);
  if (hit) return hit;
  try {
    const svg = await (await fetch(svgUrl(icon.slice(3), "regular"))).text();
    const made = await call("icon_from_svg", { svg });
    if ("icon" in made) {
      iconCache.set(icon, made.icon);
      return made.icon;
    }
  } catch {
    // no icon: the key gets a square (insertSet's fallback)
  }
  return null;
}

/** Every Phosphor icon that the given items put on keys. */
function phosphorIconsOf(items: CatalogItem[]): string[] {
  const out = new Set<string>();
  for (const item of items) {
    if (item.icon.startsWith("ph:")) out.add(item.icon);
    for (const state of Object.values(item.bind?.states ?? {})) if (state.icon?.startsWith("ph:")) out.add(state.icon);
  }
  return [...out];
}

/** The items a set or a widget refers to (`@id` options) are not keys with icons of their own: only the set's items count. */
export function itemsOfSet(set: CatalogSet): CatalogItem[] {
  return set.rows.flat().filter((id): id is string => id !== null).map((id) => catalogItem(id)).filter((i): i is CatalogItem => i !== undefined);
}

/** The context of `insertSet` / `insertItem`: language, accent, the converted icons and, for OBS items, the scenes and inputs. */
export async function insertContext(items: CatalogItem[], layout: LayoutFile, obs: { scenes: string[]; inputs: string[] } | null): Promise<InsertContext> {
  const icons: Record<string, string> = {};
  for (const name of phosphorIconsOf(items)) {
    const made = await phosphorIcon(name);
    if (made) icons[name] = made;
  }
  return {
    language: getLanguage(),
    accent: layout.display.accent ?? "#7c5cff",
    icons,
    ...(obs ? { scenes: obs.scenes, inputs: obs.inputs } : {}),
  };
}

/** The categories of the library without the empty ones (`Apps` is the installed programs, shown in its own tab). */
export function libraryCategories() {
  return CATALOG.categories.filter((c) => c.items.length > 0);
}
