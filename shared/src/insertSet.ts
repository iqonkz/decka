import type { Action, ActionsFile } from "./actions";
import { type ButtonItem, type LayoutFile, type LayoutItem, type Page } from "./layout";
import { catalogItem, CATALOG, type CatalogFile, type CatalogItem, type CatalogSet } from "./catalog";
import { sizeAllowed } from "./validate";

// Putting catalog items and sets on a page (DATA_MODEL.md 9, DESIGN.md 4.5 / 4.8). Pure: nothing is changed in the arguments.

const MAX_PAGES = 10;
const MAX_NAME = 30;

/** The same thing as `action`, ignoring `name`: an existing action with this content is reused instead of added again. */
export function findSameAction(actions: ActionsFile, action: Action): string | undefined {
  const content = (a: Action) => {
    const { name: _name, ...rest } = a;
    return JSON.stringify(Object.fromEntries(Object.entries(rest).sort(([x], [y]) => x.localeCompare(y))));
  };
  const wanted = content(action);
  return Object.entries(actions.actions).find(([, a]) => content(a) === wanted)?.[0];
}

/** `Spotify` -> `spotify`; `[a-z0-9-]`, at most 48 characters; `-2`, `-3`... when taken. */
export function makeActionId(name: string, existing: Record<string, unknown>): string {
  const slug = name.toLowerCase().replace(/[^a-z0-9]+/g, "-").replace(/^-+|-+$/g, "").slice(0, 44).replace(/-+$/, "") || "action";
  if (!(slug in existing)) return slug;
  for (let n = 2; ; n++) {
    const id = `${slug}-${n}`;
    if (!(id in existing)) return id;
  }
}

/** What the person answered for an item with `ask`. */
export interface Answer { url?: string; keys?: string[]; scene?: string; input?: string; page?: string; }

export interface InsertContext {
  language: "en" | "ru";
  /** The scenes and audio inputs of OBS (`obs_status()`), in order: a set takes them one by one. */
  scenes?: string[];
  inputs?: string[];
  /** `display.accent`: replaces `$accent` in the colours of states. */
  accent?: string;
  /** `ph:<name>` -> `img:<hash16>`, made by the window (`icon_from_svg`) before the call; an icon not in the map becomes `lucide:square`. */
  icons?: Record<string, string>;
  catalog?: CatalogFile;
  random?: () => number;
}

export interface InsertResult {
  layout: LayoutFile;
  actions: ActionsFile;
  /** The pages that got something, in order. */
  placedOn: string[];
  /** Items that were not put anywhere, with the reason. */
  skipped: { id: string; reason: "no_answer" | "no_room" | "unknown" }[];
  /** The ids of the items that were added (the window selects them). */
  added: string[];
}

export type InsertOutcome = ({ ok: true } & InsertResult) | { ok: false; error: string };

const DEFAULT_ACCENT = "#7c5cff";
const given = (s: string | undefined): s is string => s !== undefined && s !== "";

function newItemId(kind: string, random: () => number, taken: Set<string>): string {
  const chars = "abcdefghijklmnopqrstuvwxyz0123456789";
  for (;;) {
    let suffix = "";
    for (let i = 0; i < 6; i++) suffix += chars[Math.floor(random() * chars.length)] ?? "a";
    const id = `${kind === "button" ? "b" : "w"}-${suffix}`;
    if (!taken.has(id)) return id;
  }
}

interface Cell { x: number; y: number; w: number; h: number; }

/** The entries of a shape: where each named item starts (`dx`, `dy`) and how big it is. */
function shapeOf(rows: (string | null)[][], catalog: CatalogFile): { entries: { item: CatalogItem; dx: number; dy: number; w: number; h: number }[]; unknown: string[] } {
  const entries: { item: CatalogItem; dx: number; dy: number; w: number; h: number }[] = [];
  const unknown: string[] = [];
  rows.forEach((row, dy) =>
    row.forEach((id, dx) => {
      if (id === null) return;
      const item = catalogItem(id, catalog);
      if (!item) return void unknown.push(id);
      entries.push({ item, dx, dy, w: item.widget?.w ?? 1, h: item.widget?.h ?? 1 });
    }),
  );
  return { entries, unknown };
}

function overlaps(a: Cell, b: Cell): boolean {
  return a.x < b.x + b.w && b.x < a.x + a.w && a.y < b.y + b.h && b.y < a.y + a.h;
}

function fits(page: Page, grid: { cols: number; rows: number }, cell: Cell, extra: Cell[] = []): boolean {
  if (cell.x < 0 || cell.y < 0 || cell.x + cell.w > grid.cols || cell.y + cell.h > grid.rows) return false;
  return !page.items.some((i) => overlaps(cell, i)) && !extra.some((c) => overlaps(cell, c));
}

function firstSpot(page: Page, grid: { cols: number; rows: number }, w: number, h: number, extra: Cell[] = []): { x: number; y: number } | null {
  for (let y = 0; y + h <= grid.rows; y++) for (let x = 0; x + w <= grid.cols; x++) if (fits(page, grid, { x, y, w, h }, extra)) return { x, y };
  return null;
}

function freeCells(page: Page, grid: { cols: number; rows: number }): number {
  let free = 0;
  for (let y = 0; y < grid.rows; y++) for (let x = 0; x < grid.cols; x++) if (fits(page, grid, { x, y, w: 1, h: 1 })) free++;
  return free;
}

function overflowId(layout: LayoutFile, base: string): string {
  const taken = new Set(layout.pages.map((p) => p.id));
  const slug = base.toLowerCase().replace(/[^a-z0-9]+/g, "-").replace(/^-+|-+$/g, "").slice(0, 40) || "set";
  if (!taken.has(slug)) return slug;
  for (let n = 2; ; n++) if (!taken.has(`${slug}-${n}`)) return `${slug}-${n}`;
}

/** Turns an item into what goes into the files; `null` when something it asks for was not given. */
function materialize(
  item: CatalogItem,
  answer: Answer,
  ctx: Required<Pick<InsertContext, "language" | "accent">> & { icons: Record<string, string>; catalog: CatalogFile },
  actions: ActionsFile,
  newActions: Record<string, Action>,
): { layoutItem: Omit<LayoutItem, "x" | "y" | "id">; actionId?: string } | null {
  const lang = ctx.language;
  const icon = (i: string): string => (i.startsWith("ph:") ? (ctx.icons[i] ?? "lucide:square") : i);
  const colour = (c: string | undefined): string | undefined => (c === "$accent" ? ctx.accent : c);

  /** The id of the action of `source` (an existing equal action is reused), the action added when there is none. */
  const actionIdFor = (source: CatalogItem, a: Answer): string | undefined => {
    if (!source.action) return undefined;
    let body = { ...source.action } as Record<string, unknown>;
    if (source.ask === "url") body = { ...body, url: a.url };
    if (source.ask === "keys") body = { ...body, keys: a.keys };
    if (source.ask === "obs_scene") body = { ...body, scene: a.scene };
    if (source.ask === "obs_input") body = { ...body, input: a.input };
    const action = { name: source.name[lang].slice(0, 60), ...body } as Action;
    const all: ActionsFile = { ...actions, actions: { ...actions.actions, ...newActions } };
    const same = findSameAction(all, action);
    if (same) return same;
    // Ids come from the English name so that they are the same in both languages.
    const id = makeActionId(source.name.en, all.actions);
    newActions[id] = action;
    return id;
  };

  if (item.ask === "url" && !given(answer.url)) return null;
  if (item.ask === "keys" && !(answer.keys && answer.keys.length > 0)) return null;
  if (item.ask === "obs_scene" && !given(answer.scene)) return null;
  if (item.ask === "obs_input" && !given(answer.input)) return null;
  if (item.ask === "page" && !given(answer.page)) return null;
  if (item.ask === "sequence") return null; // made by the sequence editor, not here

  if (item.widget) {
    const options: Record<string, unknown> = {};
    for (const [key, value] of Object.entries(item.widget.options ?? {})) {
      if (typeof value === "string" && value.startsWith("@")) {
        const source = catalogItem(value.slice(1), ctx.catalog);
        const id = source ? actionIdFor(source, {}) : undefined;
        if (id !== undefined) options[key] = id;
      } else {
        options[key] = value;
      }
    }
    return { layoutItem: { kind: item.widget.kind, w: item.widget.w, h: item.widget.h, ...options } as Omit<LayoutItem, "x" | "y" | "id"> };
  }

  const actionId = actionIdFor(item, answer);
  const look: Partial<ButtonItem> = { label: item.label[lang], icon: icon(item.icon) };
  const color = colour(item.color);
  if (color !== undefined) look.color = color;
  const button: Record<string, unknown> = { kind: "button", w: 1, h: 1, ...look };
  if (actionId !== undefined) button.actionId = actionId;
  if (item.confirm) button.confirm = true;
  if (item.navigate !== undefined) button.navigate = item.navigate;
  if (item.ask === "page") button[item.popup ? "popup" : "navigate"] = answer.page;
  if (item.bind) {
    const states: Record<string, Record<string, string>> = {};
    for (const [key, state] of Object.entries(item.bind.states)) {
      const shown: Record<string, string> = {};
      if (state.label) shown.label = state.label[lang];
      if (state.icon) shown.icon = icon(state.icon);
      const c = colour(state.color);
      if (c !== undefined) shown.color = c;
      states[key === "$scene" ? (answer.scene ?? key) : key] = shown;
    }
    button.bind = { var: item.bind.var, states };
  }
  return { layoutItem: button as Omit<LayoutItem, "x" | "y" | "id">, ...(actionId !== undefined ? { actionId } : {}) };
}

function withContext(ctx: InsertContext) {
  return { language: ctx.language, accent: ctx.accent ?? DEFAULT_ACCENT, icons: ctx.icons ?? {}, catalog: ctx.catalog ?? CATALOG };
}

/**
 * A set on a page (DATA_MODEL.md 9, rules 1-6):
 * the whole shape at the first free place; else the items one by one into the free cells; else a new page after this one.
 * Items that ask for something take it from `ctx` (OBS scenes in order) and are skipped without it.
 */
export function insertSet(layout: LayoutFile, actions: ActionsFile, pageId: string, set: CatalogSet, ctx: InsertContext): InsertOutcome {
  const catalog = ctx.catalog ?? CATALOG;
  const { entries, unknown } = shapeOf(set.rows, catalog);
  // Answers of the items that ask: scenes and inputs of OBS are taken in order, one per item.
  let scene = 0;
  let input = 0;
  const answers = entries.map(({ item }): Answer => {
    if (item.ask === "obs_scene") return ctx.scenes?.[scene++] !== undefined ? { scene: ctx.scenes[scene - 1]! } : {};
    if (item.ask === "obs_input") return ctx.inputs?.[input++] !== undefined ? { input: ctx.inputs[input - 1]! } : {};
    return {};
  });
  return place(layout, actions, pageId, entries.map((e, i) => ({ ...e, answer: answers[i]! })), unknown, ctx, { newPage: { name: set.name[ctx.language] } });
}

/** One catalog item at the first free cell of a page (a double click in the library), or at `cell`. */
export function insertItem(layout: LayoutFile, actions: ActionsFile, pageId: string, itemId: string, answer: Answer, ctx: InsertContext, cell?: { x: number; y: number }): InsertOutcome {
  const catalog = ctx.catalog ?? CATALOG;
  const item = catalogItem(itemId, catalog);
  if (!item) return { ok: false, error: `unknown catalog item '${itemId}'` };
  const w = item.widget?.w ?? 1;
  const h = item.widget?.h ?? 1;
  return place(layout, actions, pageId, [{ item, dx: 0, dy: 0, w, h, answer }], [], ctx, cell ? { at: cell } : { single: true });
}

interface Placed { item: CatalogItem; dx: number; dy: number; w: number; h: number; answer: Answer; }

function place(
  layout: LayoutFile,
  actions: ActionsFile,
  pageId: string,
  entries: Placed[],
  unknown: string[],
  ctx: InsertContext,
  mode: { newPage: { name: string } } | { single: true } | { at: { x: number; y: number } },
): InsertOutcome {
  const pageIndex = layout.pages.findIndex((p) => p.id === pageId);
  if (pageIndex < 0) return { ok: false, error: `no page '${pageId}'` };
  const full = withContext(ctx);
  const random = ctx.random ?? Math.random;
  const skipped: InsertResult["skipped"] = unknown.map((id) => ({ id, reason: "unknown" as const }));

  // Materialize first: items without their answer drop out (rule 5), and the shape stays as it is drawn.
  const newActions: Record<string, Action> = {};
  const made: { entry: Placed; built: NonNullable<ReturnType<typeof materialize>> }[] = [];
  for (const entry of entries) {
    const built = materialize(entry.item, entry.answer, full, actions, newActions);
    if (built === null) skipped.push({ id: entry.item.id, reason: "no_answer" });
    else made.push({ entry, built });
  }
  if (made.length === 0) return { ok: true, layout, actions, placedOn: [], skipped, added: [] };

  const grid = layout.grid;
  const page = layout.pages[pageIndex]!;
  const taken = new Set(layout.pages.flatMap((p) => p.items.map((i) => i.id)));
  const pages = [...layout.pages];
  const placedOn: string[] = [];
  const added: string[] = [];

  const put = (target: Page, entry: Placed, built: (typeof made)[number]["built"], at: { x: number; y: number }): Page => {
    const id = newItemId(entry.item.widget ? "w" : "b", random, taken);
    taken.add(id);
    added.push(id);
    const item = { id, ...built.layoutItem, x: at.x, y: at.y } as LayoutItem;
    return { ...target, items: [...target.items, item] };
  };
  const sizeOk = (m: (typeof made)[number], g: { cols: number; rows: number }) => !m.entry.item.widget || sizeAllowed(m.entry.item.widget.kind, m.entry.w, m.entry.h, g, m.entry.item.widget.options?.graph === true);

  // A single spot given by the caller.
  if ("at" in mode) {
    const m = made[0]!;
    if (!sizeOk(m, grid) || !fits(page, grid, { ...mode.at, w: m.entry.w, h: m.entry.h })) return { ok: false, error: "No free cell on this page" };
    pages[pageIndex] = put(page, m.entry, m.built, mode.at);
    return { ok: true, layout: { ...layout, pages }, actions: mergeActions(actions, newActions), placedOn: [pageId], skipped, added };
  }

  // Rule 2: the whole shape at the first place where it fits.
  const shapeW = Math.max(...made.map((m) => m.entry.dx + m.entry.w));
  const shapeH = Math.max(...made.map((m) => m.entry.dy + m.entry.h));
  const cellsFor = (origin: { x: number; y: number }) => made.map((m) => ({ x: origin.x + m.entry.dx, y: origin.y + m.entry.dy, w: m.entry.w, h: m.entry.h }));
  const wholeAt = (target: Page): { x: number; y: number } | null => {
    for (let y = 0; y + shapeH <= grid.rows; y++) {
      for (let x = 0; x + shapeW <= grid.cols; x++) {
        const cells = cellsFor({ x, y });
        if (cells.every((c, i) => fits(target, grid, c, cells.slice(0, i)) && sizeOk(made[i]!, grid))) return { x, y };
      }
    }
    return null;
  };
  const origin = wholeAt(page);
  if (origin) {
    let next = page;
    made.forEach((m, i) => (next = put(next, m.entry, m.built, { x: origin.x + m.entry.dx, y: origin.y + m.entry.dy })));
    pages[pageIndex] = next;
    return { ok: true, layout: { ...layout, pages }, actions: mergeActions(actions, newActions), placedOn: [pageId], skipped, added };
  }

  // Rule 3: the items one by one into the free cells, when there are enough of them (widgets: the first place they fit).
  const needed = made.reduce((n, m) => n + m.entry.w * m.entry.h, 0);
  const oneByOne = (target: Page): Page | null => {
    let next = target;
    for (const m of made) {
      const spot = sizeOk(m, grid) ? firstSpot(next, grid, m.entry.w, m.entry.h) : null;
      if (!spot) {
        skipped.push({ id: m.entry.item.id, reason: "no_room" });
        continue;
      }
      next = put(next, m.entry, m.built, spot);
    }
    return next;
  };
  const alone = "single" in mode;
  if (!alone && freeCells(page, grid) >= needed) {
    pages[pageIndex] = oneByOne(page)!;
    return { ok: true, layout: { ...layout, pages }, actions: mergeActions(actions, newActions), placedOn: [pageId], skipped, added };
  }
  if (alone) {
    // A single item: the first free place or nothing.
    const m = made[0]!;
    const spot = sizeOk(m, grid) ? firstSpot(page, grid, m.entry.w, m.entry.h) : null;
    if (!spot) return { ok: false, error: "No free cell on this page" };
    pages[pageIndex] = put(page, m.entry, m.built, spot);
    return { ok: true, layout: { ...layout, pages }, actions: mergeActions(actions, newActions), placedOn: [pageId], skipped, added };
  }

  // Rule 4: a new page with the name of the set, right after this one; the shape from the top left corner.
  if (layout.pages.length >= MAX_PAGES) return { ok: false, error: "Page limit reached" };
  const name = (mode as { newPage: { name: string } }).newPage.name.slice(0, MAX_NAME);
  const id = overflowId(layout, name);
  let fresh: Page = { id, name, items: [] };
  if (wholeAt(fresh)) {
    const spot = wholeAt(fresh)!;
    made.forEach((m) => (fresh = put(fresh, m.entry, m.built, { x: spot.x + m.entry.dx, y: spot.y + m.entry.dy })));
  } else {
    for (const m of made) {
      const spot = sizeOk(m, grid) ? firstSpot(fresh, grid, m.entry.w, m.entry.h) : null;
      if (!spot) skipped.push({ id: m.entry.item.id, reason: "no_room" });
      else fresh = put(fresh, m.entry, m.built, spot);
    }
  }
  pages.splice(pageIndex + 1, 0, fresh);
  placedOn.push(id);
  return { ok: true, layout: { ...layout, pages }, actions: mergeActions(actions, newActions), placedOn, skipped, added };
}

function mergeActions(actions: ActionsFile, added: Record<string, Action>): ActionsFile {
  return Object.keys(added).length === 0 ? actions : { ...actions, actions: { ...actions.actions, ...added } };
}
