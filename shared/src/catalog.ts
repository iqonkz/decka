import type { Action } from "./actions";
import type { LayoutItem } from "./layout";
import catalogJson from "../catalog/catalog.json";
import setsJson from "../catalog/sets.json";

// The catalog of ready-made keys and the sets (DATA_MODEL.md 9). Data, not code: shared/catalog/*.json. The phone never sees it.

export interface Bilingual { en: string; ru: string; }

/** An action without its name: the name comes from the catalog item (in the language of the window). */
export type ActionBody = Action extends infer A ? (A extends unknown ? Omit<A, "name"> : never) : never;

export interface CatalogBindState {
  label?: Bilingual;
  icon?: string;
  /** `#rrggbb`, or `$accent`: the accent colour of the deck (`display.accent`) at the moment the key is made. */
  color?: string;
}

export type CatalogAsk = "url" | "keys" | "obs_scene" | "obs_input" | "page" | "sequence" | "text";
export type Integration = "obs" | "weather" | "twitch";

export interface CatalogItem {
  /** `media.shuffle`, `sound.mic`: unique. */
  id: string;
  /** The label of the key, at most 10 characters in both languages. */
  label: Bilingual;
  /** The name in the library and in actions.json, at most 40. */
  name: Bilingual;
  /** `lucide:<name>` or `ph:<name>` (Phosphor: the window turns it into `img:` when it makes the key). */
  icon: string;
  /** Only for services and dangerous actions (DESIGN.md 2.4). */
  color?: string;
  action?: ActionBody;
  /** The key is pressed by holding (Lock, Sleep, Go live). */
  confirm?: boolean;
  /** What is missing: the window asks for it. For `obs_scene` the key's bind state `$scene` is the chosen scene. */
  ask?: CatalogAsk;
  /** With `ask: "page"`: the key opens a popup group, not a page. */
  popup?: boolean;
  bind?: { var: string; states: Record<string, CatalogBindState> };
  navigate?: "$back";
  widget?: { kind: LayoutItem["kind"]; w: number; h: number; /** Values `@<item id>` become the id of that item's action. */ options?: Record<string, unknown> };
  requires?: Integration;
}

export interface CatalogCategory { id: string; name: Bilingual; icon: string; items: string[]; }

export interface CatalogFile { version: 1; categories: CatalogCategory[]; items: CatalogItem[]; }

export interface CatalogSet {
  id: string;
  name: Bilingual;
  description: Bilingual;
  icon: string;
  /** Item ids by row; `null` is an empty cell, and the cells a widget covers are `null` too. A widget starts at the cell where it is named. */
  rows: (string | null)[][];
  requires?: Integration;
}

export interface SetsFile { version: 1; sets: CatalogSet[]; }

export const CATALOG = catalogJson as unknown as CatalogFile;
export const SETS = (setsJson as unknown as SetsFile).sets;

export const LABEL_MAX = 10;

export function catalogItem(id: string, catalog: CatalogFile = CATALOG): CatalogItem | undefined {
  return catalog.items.find((i) => i.id === id);
}

/** The items of a category in its order. */
export function categoryItems(category: CatalogCategory, catalog: CatalogFile = CATALOG): CatalogItem[] {
  return category.items.map((id) => catalogItem(id, catalog)).filter((i): i is CatalogItem => i !== undefined);
}

/** Library search: the query is looked for in the names and labels of both languages and in the category names (DESIGN.md 4.8). */
export function searchCatalog(query: string, catalog: CatalogFile = CATALOG): CatalogItem[] {
  const q = query.trim().toLowerCase();
  if (q === "") return [];
  const hit = (b: Bilingual) => b.en.toLowerCase().includes(q) || b.ru.toLowerCase().includes(q);
  const inCategory = new Set<string>();
  for (const c of catalog.categories) if (hit(c.name)) c.items.forEach((id) => inCategory.add(id));
  return catalog.items.filter((i) => hit(i.name) || hit(i.label) || inCategory.has(i.id));
}
