import { t, type Action, type ActionsFile, type InstalledApp, type LayoutFile, type LayoutItem, type Page } from "@decka/shared";

/** Grid size of the layout being edited (DATA_MODEL.md 3.1: cols 3..8, rows 2..5). */
export interface Grid {
  cols: number;
  rows: number;
}
export const COLS_RANGE = [3, 8] as const;
export const ROWS_RANGE = [2, 5] as const;

export type Kind = LayoutItem["kind"];

/** Size of a newly created item of each kind. */
export const DEFAULT_SIZE: Record<Kind, { w: number; h: number }> = {
  button: { w: 1, h: 1 },
  clock: { w: 2, h: 1 },
  sys: { w: 2, h: 1 },
  status: { w: 1, h: 1 },
  statuses: { w: 2, h: 2 },
  notifications: { w: 2, h: 2 },
  media: { w: 3, h: 1 },
  volume: { w: 3, h: 1 },
  timer: { w: 2, h: 1 },
  weather: { w: 2, h: 1 },
  obs: { w: 2, h: 1 },
  ai: { w: 3, h: 2 },
  twitch: { w: 3, h: 1 },
  youtube: { w: 3, h: 1 },
  battery: { w: 2, h: 1 },
  mail: { w: 2, h: 1 },
  context: { w: 3, h: 2 },
};

/** Allowed sizes per kind (DATA_MODEL.md 3.2 item 4): [minW, maxW, minH, maxH]; list widgets may fill the grid. */
export function sizeRange(kind: Kind, grid: Grid): [number, number, number, number] {
  switch (kind) {
    case "button":
    case "status":
      return [1, 1, 1, 1];
    case "clock":
      return [1, 2, 1, 1];
    case "sys":
      return [2, 2, 1, 2];
    case "statuses":
    case "notifications":
    case "context":
      return [2, grid.cols, 1, grid.rows];
    case "media":
      return [2, grid.cols, 1, 2];
    case "volume":
      return [1, 4, 1, 4];
    case "timer":
      return [1, 2, 1, 2];
    case "weather":
      return [1, 4, 1, 2];
    case "obs":
    case "ai":
    case "twitch":
    case "youtube":
      return [2, 4, 1, 2];
    case "battery":
      return [1, 3, 1, 2];
    case "mail":
      return [1, 2, 1, 2];
  }
}

interface Rect {
  x: number;
  y: number;
  w: number;
  h: number;
}

const overlap = (a: Rect, b: Rect): boolean => a.x < b.x + b.w && b.x < a.x + a.w && a.y < b.y + b.h && b.y < a.y + a.h;

/** True if the rectangle lies inside the grid and overlaps no other item of the page. */
export function fits(page: Page, grid: Grid, rect: Rect, ignoreId?: string): boolean {
  if (rect.x < 0 || rect.y < 0 || rect.w < 1 || rect.h < 1 || rect.x + rect.w > grid.cols || rect.y + rect.h > grid.rows) return false;
  return page.items.every((item) => item.id === ignoreId || !overlap(rect, item));
}

/** First free position (row by row) for a w x h item, or null. */
export function findFreeSpot(page: Page, grid: Grid, w: number, h: number): { x: number; y: number } | null {
  for (let y = 0; y + h <= grid.rows; y++) {
    for (let x = 0; x + w <= grid.cols; x++) {
      if (fits(page, grid, { x, y, w, h })) return { x, y };
    }
  }
  return null;
}

export const NO_FREE_CELL = "No free cell on this page";

/** The `+` of the library (DESIGN.md 4.8a): puts the item into the first free place of the page; its x and y are replaced. */
export function addToFirstFree(layout: LayoutFile, pageId: string, item: LayoutItem): { ok: true; layout: LayoutFile } | { ok: false; error: string } {
  const page = layout.pages.find((p) => p.id === pageId);
  if (!page) return { ok: false, error: NO_FREE_CELL };
  const spot = findFreeSpot(page, layout.grid, item.w, item.h);
  if (!spot) return { ok: false, error: NO_FREE_CELL };
  const placed = { ...item, x: spot.x, y: spot.y } as LayoutItem;
  return { ok: true, layout: { ...layout, pages: layout.pages.map((p) => (p.id === pageId ? { ...p, items: [...p.items, placed] } : p)) } };
}

export type AddResult ={ ok: true; page: Page } | { ok: false; error: string };

export const NO_SPACE = "Not enough free space here";

export function addItem(page: Page, grid: Grid, item: LayoutItem): AddResult {
  if (!fits(page, grid, item)) return { ok: false, error: t("add.no_space") };
  return { ok: true, page: { ...page, items: [...page.items, item] } };
}

/** Shallow merge of `patch` into the item with that id (validity is checked later by validateLayout). */
export function updateItem(page: Page, id: string, patch: Partial<LayoutItem>): Page {
  return { ...page, items: page.items.map((i) => (i.id === id ? ({ ...i, ...patch } as LayoutItem) : i)) };
}

export function removeItem(page: Page, id: string): Page {
  return { ...page, items: page.items.filter((i) => i.id !== id) };
}

/** Moves a page by `delta` positions (-1 = left, +1 = right); out-of-range moves change nothing. */
export function movePage(layout: LayoutFile, index: number, delta: number): LayoutFile {
  const target = index + delta;
  if (index < 0 || index >= layout.pages.length || target < 0 || target >= layout.pages.length) return layout;
  const pages = [...layout.pages];
  const [moved] = pages.splice(index, 1);
  if (!moved) return layout;
  pages.splice(target, 0, moved);
  return { ...layout, pages };
}

/** `b-xxxxxx` for buttons, `w-xxxxxx` for widgets. */
export function newItemId(kind: Kind, random: () => number = Math.random): string {
  const chars = "abcdefghijklmnopqrstuvwxyz0123456789";
  let suffix = "";
  for (let i = 0; i < 6; i++) suffix += chars[Math.floor(random() * chars.length)] ?? "a";
  return `${kind === "button" ? "b" : "w"}-${suffix}`;
}

/** A new item of `kind` with its default size at (x, y). */
export function makeItem(kind: Kind, x: number, y: number, id = newItemId(kind)): LayoutItem {
  const { w, h } = DEFAULT_SIZE[kind];
  const base = { id, x, y, w, h };
  switch (kind) {
    case "button":
      return { ...base, kind, label: t("item.new_button") };
    case "clock":
      return { ...base, kind };
    case "sys":
      return { ...base, kind };
    case "status":
      return { ...base, kind, statusId: "" };
    case "statuses":
      return { ...base, kind };
    case "notifications":
      return { ...base, kind };
    case "media":
    case "volume":
    case "timer":
    case "weather":
    case "obs":
    case "ai":
    case "twitch":
    case "youtube":
    case "battery":
    case "mail":
    case "context":
      return { ...base, kind };
  }
}

/** `Notepad` -> `app-notepad`; the first free id when it is taken. */
export function newAppActionId(name: string, taken: Record<string, unknown>): string {
  const base = `app-${name.toLowerCase().replace(/[^a-z0-9]+/g, "-").replace(/^-+|-+$/g, "").slice(0, 50) || "program"}`;
  if (!(base in taken)) return base;
  for (let n = 2; ; n++) if (!(`${base}-${n}` in taken)) return `${base}-${n}`;
}

/** The launch action of an installed program (DATA_MODEL.md 2). */
export function launchActionOf(app: InstalledApp): Action {
  return { name: app.name.slice(0, 60), type: "launch", program: app.program, ...(app.args.length > 0 ? { args: app.args } : {}), ...(app.cwd ? { cwd: app.cwd } : {}) };
}

/**
 * Adds the chosen programs to the first page: one launch action and one button each, in the free cells in reading order.
 * `icons` maps a program path (lower case) to its `img:` icon, when one could be made. Programs that find no free
 * cell are counted in `skipped`.
 */
export function planAppButtons(
  selected: InstalledApp[],
  layout: LayoutFile,
  actions: ActionsFile,
  icons: Record<string, string | undefined> = {},
): { layout: LayoutFile; actions: ActionsFile; placed: number; skipped: number } {
  const first = layout.pages[0];
  if (!first) return { layout, actions, placed: 0, skipped: selected.length };
  let page = first;
  const nextActions: ActionsFile = { ...actions, actions: { ...actions.actions } };
  let placed = 0;
  for (const app of selected) {
    const spot = findFreeSpot(page, layout.grid, 1, 1);
    if (!spot) continue;
    const actionId = newAppActionId(app.name, nextActions.actions);
    nextActions.actions[actionId] = launchActionOf(app);
    const icon = icons[app.program.toLowerCase()];
    const item = { ...makeItem("button", spot.x, spot.y), label: app.name.slice(0, 24), actionId, ...(icon ? { icon } : {}) } as LayoutItem;
    page = { ...page, items: [...page.items, item] };
    placed++;
  }
  return { layout: { ...layout, pages: layout.pages.map((p, i) => (i === 0 ? page : p)) }, actions: nextActions, placed, skipped: selected.length - placed };
}

/** What a button does on a tap (DESIGN.md 4.1): the four combinations rule 12 allows. */
export type TapMode = "action" | "page" | "popup" | "action-popup";

export const tapModeLabels = (): Record<TapMode, string> => ({
  action: t("tap.action"),
  page: t("tap.page"),
  popup: t("tap.popup"),
  "action-popup": t("tap.action_popup"),
});

export function tapModeOf(item: { actionId?: string; navigate?: string; popup?: string }): TapMode {
  if (item.popup !== undefined) return item.actionId !== undefined ? "action-popup" : "popup";
  return item.navigate !== undefined ? "page" : "action";
}

/** The fields to change when the user picks `mode`; others are cleared (`undefined`). */
export function tapModePatch(
  item: { actionId?: string },
  mode: TapMode,
  options: { firstActionId: string | undefined; firstPageId: string | undefined; firstPopupId: string | undefined },
): Record<string, unknown> {
  const action = item.actionId ?? options.firstActionId;
  switch (mode) {
    case "action":
      return { navigate: undefined, popup: undefined, actionId: action };
    case "page":
      return { actionId: undefined, popup: undefined, confirm: undefined, navigate: options.firstPageId };
    case "popup":
      return { actionId: undefined, navigate: undefined, confirm: undefined, popup: options.firstPopupId };
    case "action-popup":
      return { navigate: undefined, actionId: action, popup: options.firstPopupId };
  }
}

/** The page list a "page" button may point at (popup groups are not pages to go to). */
export const normalPages = (pages: Page[]): Page[] => pages.filter((p) => p.popup !== true);
export const popupPages = (pages: Page[]): Page[] => pages.filter((p) => p.popup === true);

/** A new popup group page, appended after the others. */
export function newPopupPage(pages: Page[], random: () => number = Math.random): Page {
  const n = popupPages(pages).length + 1;
  return { id: newPageId(random), name: t("page.group_n", { n }), popup: true, items: [] };
}

/** What the user typed -> an exe name: trimmed, lower case, `.exe` appended when missing. Empty input -> null. */
export function normalizeApp(input: string): string | null {
  const name = input.trim().toLowerCase();
  if (name === "") return null;
  return name.endsWith(".exe") ? name : `${name}.exe`;
}

/** The page without `apps` when the list is empty, otherwise with the list. */
export function withApps(page: Page, apps: string[]): Page {
  const { apps: _old, ...rest } = page;
  return apps.length > 0 ? { ...rest, apps } : rest;
}

/** A new `media` widget uses the standard media actions when they exist. */
export function withMediaDefaults(item: LayoutItem, actionIds: string[]): LayoutItem {
  if (item.kind !== "media") return item;
  const has = (id: string) => actionIds.includes(id);
  return {
    ...item,
    ...(has("media-prev") ? { prevAction: "media-prev" } : {}),
    ...(has("media-play") ? { playAction: "media-play" } : {}),
    ...(has("media-next") ? { nextAction: "media-next" } : {}),
  };
}

/** Page ids: `p-xxxxxx`. */
export function newPageId(random: () => number = Math.random): string {
  return `p-${newItemId("sys", random).slice(2)}`;
}
