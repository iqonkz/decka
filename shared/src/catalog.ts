import type { Action } from "./actions";
import type { LayoutItem } from "./layout";
import catalogJson from "../catalog/catalog.json";
import setsJson from "../catalog/sets.json";
import zhJson from "../catalog/i18n/zh.json";
import hiJson from "../catalog/i18n/hi.json";
import esJson from "../catalog/i18n/es.json";
import arJson from "../catalog/i18n/ar.json";
import frJson from "../catalog/i18n/fr.json";
import ptJson from "../catalog/i18n/pt.json";
import deJson from "../catalog/i18n/de.json";
import jaJson from "../catalog/i18n/ja.json";
import kkJson from "../catalog/i18n/kk.json";
import type { Language } from "./languages";
import type { LocalizedText } from "./localized";

// The catalog of ready-made keys and the sets (DATA_MODEL.md 9). Data, not code: shared/catalog/*.json. The phone never sees it.
// `catalog.json` and `sets.json` hold English and Russian; the other languages are in `catalog/i18n/<code>.json` and are merged
// into the same `LocalizedText` objects when this module loads.

/** @deprecated the texts have all the languages of M19: use `LocalizedText` */
export type Bilingual = LocalizedText;

/** An action without its name: the name comes from the catalog item (in the language of the window). */
export type ActionBody = Action extends infer A ? (A extends unknown ? Omit<A, "name"> : never) : never;

export interface CatalogBindState {
  label?: Bilingual;
  icon?: string;
  /** `#rrggbb`, or `$accent`: the accent colour of the deck (`display.accent`) at the moment the key is made. */
  color?: string;
}

export type CatalogAsk = "url" | "keys" | "obs_scene" | "obs_input" | "page" | "sequence" | "text" | "ha_entity";
export type Integration = "obs" | "weather" | "twitch" | "youtube";

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

/** What a file of `catalog/i18n/<code>.json` may hold (DATA_MODEL.md 9). */
export interface CatalogOverlay {
  categories?: Record<string, string>;
  /** `states` = the labels of the bound states of a key (`"true"` -> text). */
  items?: Record<string, { name?: string; label?: string; states?: Record<string, string> }>;
  sets?: Record<string, { name?: string; description?: string }>;
  /** Used by the agent (templates.rs): the template names and the captions of their actions. */
  templates?: { name?: Record<string, string>; actions?: Record<string, Record<string, string>> };
}

const OVERLAYS: Partial<Record<Language, CatalogOverlay>> = { zh: zhJson, hi: hiJson, es: esJson, ar: arJson, fr: frJson, pt: ptJson, de: deJson, ja: jaJson, kk: kkJson };

/** A copy of the data with the texts of every overlay added. A text the overlay does not have stays without that language. */
export function mergeOverlays(catalog: CatalogFile, sets: CatalogSet[], overlays: Partial<Record<Language, CatalogOverlay>> = OVERLAYS): { catalog: CatalogFile; sets: CatalogSet[] } {
  const merged: CatalogFile = JSON.parse(JSON.stringify(catalog)) as CatalogFile;
  const mergedSets: CatalogSet[] = JSON.parse(JSON.stringify(sets)) as CatalogSet[];
  for (const [lang, overlay] of Object.entries(overlays) as [Language, CatalogOverlay][]) {
    for (const c of merged.categories) {
      const text = overlay.categories?.[c.id];
      if (text !== undefined) c.name[lang] = text;
    }
    for (const item of merged.items) {
      const o = overlay.items?.[item.id];
      if (!o) continue;
      if (o.name !== undefined) item.name[lang] = o.name;
      if (o.label !== undefined) item.label[lang] = o.label;
      for (const [key, text] of Object.entries(o.states ?? {})) {
        const state = item.bind?.states[key];
        if (state?.label) state.label[lang] = text;
      }
    }
    for (const set of mergedSets) {
      const o = overlay.sets?.[set.id];
      if (!o) continue;
      if (o.name !== undefined) set.name[lang] = o.name;
      if (o.description !== undefined) set.description[lang] = o.description;
    }
  }
  return { catalog: merged, sets: mergedSets };
}

const merged = mergeOverlays(catalogJson as unknown as CatalogFile, (setsJson as unknown as SetsFile).sets);
export const CATALOG: CatalogFile = merged.catalog;
export const SETS: CatalogSet[] = merged.sets;

export const LABEL_MAX = 12;

export function catalogItem(id: string, catalog: CatalogFile = CATALOG): CatalogItem | undefined {
  return catalog.items.find((i) => i.id === id);
}

/** The items of a category in its order. */
export function categoryItems(category: CatalogCategory, catalog: CatalogFile = CATALOG): CatalogItem[] {
  return category.items.map((id) => catalogItem(id, catalog)).filter((i): i is CatalogItem => i !== undefined);
}

/** Does the query occur in the text in any language (the library finds a key by its English name in every interface language)? */
export function textHits(text: LocalizedText, q: string): boolean {
  return Object.values(text).some((s) => typeof s === "string" && s.toLowerCase().includes(q));
}

/** Library search: the query is looked for in the names and labels of every language and in the category names (DESIGN.md 4.8). */
export function searchCatalog(query: string, catalog: CatalogFile = CATALOG): CatalogItem[] {
  const q = query.trim().toLowerCase();
  if (q === "") return [];
  const hit = (b: LocalizedText) => textHits(b, q);
  const inCategory = new Set<string>();
  for (const c of catalog.categories) if (hit(c.name)) c.items.forEach((id) => inCategory.add(id));
  return catalog.items.filter((i) => hit(i.name) || hit(i.label) || inCategory.has(i.id));
}
