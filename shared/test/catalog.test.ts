/// <reference types="node" />
import fs from "node:fs";
import path from "node:path";
import Ajv2020 from "ajv/dist/2020";
import { describe, expect, test } from "vitest";
import catalogSchema from "../schemas/catalog.schema.json";
import { CATALOG, LABEL_MAX, SETS, catalogItem, categoryItems, searchCatalog, type CatalogItem } from "../src/catalog";
import { insertItem, insertSet, previewSet, type Answer, type InsertContext } from "../src/insertSet";
import { sizeAllowed, validateActions, validateLayout } from "../src/validate";
import type { Action, ActionsFile } from "../src/actions";
import type { LayoutFile, LayoutItem } from "../src/layout";
import catalogJson from "../catalog/catalog.json";
import setsJson from "../catalog/sets.json";
import layoutDefault from "../fixtures/layout.default.json";
import actionsDefault from "../fixtures/actions.default.json";

const ID = "https://decka.invalid/schemas/catalog.schema.json";
const ajv = new Ajv2020({ allErrors: true, strict: false });
ajv.addSchema(catalogSchema);

/** The 60 Lucide icons of the phone build (DATA_MODEL.md 3.6) and the three of M17 (rotate-ccw, log-out, monitor-off; 10.8). */
const LUCIDE = new Set(
  "play pause skip-back skip-forward volume volume-1 volume-2 volume-x mic mic-off video video-off monitor lock power moon sun home settings terminal code link git-branch globe folder file-text activity cpu hard-drive wifi bell bell-off mail message-square music headphones camera image calendar clock timer star heart zap rocket bug check x plus minus refresh-cw search download upload copy trash-2 square circle arrow-left arrow-right rotate-ccw log-out monitor-off".split(" "),
);
const PHOSPHOR = path.resolve(__dirname, "../../node_modules/@phosphor-icons/core/assets/regular");

const base = layoutDefault as LayoutFile;
const actionsBase = actionsDefault as ActionsFile;
const grid = (cols: number, rows: number) => ({ cols, rows });

describe("the catalog files", () => {
  test("they match the JSON schema", () => {
    const check = (name: string, data: unknown) => {
      const validate = ajv.getSchema(`${ID}#/$defs/${name}`)!;
      const ok = validate(data);
      if (!ok) console.error(validate.errors);
      expect(ok, name).toBe(true);
    };
    check("CatalogFile", catalogJson);
    check("SetsFile", setsJson);
  });

  test("ids are unique and every reference points at something", () => {
    const ids = CATALOG.items.map((i) => i.id);
    expect(new Set(ids).size).toBe(ids.length);
    expect(new Set(SETS.map((s) => s.id)).size).toBe(SETS.length);
    expect(new Set(CATALOG.categories.map((c) => c.id)).size).toBe(CATALOG.categories.length);
    const inCategory = CATALOG.categories.flatMap((c) => c.items);
    for (const id of inCategory) expect(catalogItem(id), `category item ${id}`).toBeDefined();
    for (const id of ids) expect(inCategory, `${id} is in no category`).toContain(id);
    for (const set of SETS) for (const id of set.rows.flat()) if (id !== null) expect(catalogItem(id), `${set.id}: ${id}`).toBeDefined();
  });

  test("labels fit a key: at most 10 characters in both languages, also in the states", () => {
    for (const item of CATALOG.items) {
      for (const lang of ["en", "ru"] as const) {
        expect(item.label[lang].length, `${item.id} ${lang}`).toBeLessThanOrEqual(LABEL_MAX);
        expect(item.name[lang].length, `${item.id} name ${lang}`).toBeLessThanOrEqual(40);
        for (const [key, state] of Object.entries(item.bind?.states ?? {})) if (state.label) expect(state.label[lang].length, `${item.id} state ${key} ${lang}`).toBeLessThanOrEqual(LABEL_MAX);
      }
    }
  });

  test("every action passes the validation of actions", () => {
    const actions: ActionsFile = { version: 1, actions: {} };
    for (const item of CATALOG.items) if (item.action) actions.actions[item.id.replace(/\./g, "-")] = { name: item.name.en, ...item.action } as Action;
    expect(validateActions(actions)).toEqual([]);
    expect(Object.keys(actions.actions).length).toBeGreaterThan(20);
  });

  test("icons exist: Lucide ones in the phone build, Phosphor ones in the package; states use Lucide only", () => {
    const icons = (item: CatalogItem) => [item.icon, ...Object.values(item.bind?.states ?? {}).map((s) => s.icon).filter((i): i is string => i !== undefined)];
    for (const item of CATALOG.items) {
      for (const icon of icons(item)) {
        const [kind, name] = icon.split(":") as [string, string];
        if (kind === "lucide") expect(LUCIDE.has(name), `${item.id}: ${icon}`).toBe(true);
        else expect(fs.existsSync(path.join(PHOSPHOR, `${name}.svg`)), `${item.id}: ${icon}`).toBe(true);
      }
      for (const state of Object.values(item.bind?.states ?? {})) if (state.icon) expect(state.icon.startsWith("lucide:"), `${item.id} state icon`).toBe(true);
    }
    for (const set of SETS) expect(set.icon.startsWith("lucide:")).toBe(true);
  });

  test("widget sizes are allowed and their @options point at items with an action", () => {
    for (const item of CATALOG.items) {
      if (!item.widget) continue;
      const { kind, w, h, options } = item.widget;
      for (const g of [grid(7, 4), grid(4, 7)]) expect(sizeAllowed(kind, w, h, g, options?.graph === true), `${item.id} on ${g.cols}x${g.rows}`).toBe(true);
      for (const value of Object.values(options ?? {})) {
        if (typeof value === "string" && value.startsWith("@")) expect(catalogItem(value.slice(1))?.action, `${item.id}: ${value}`).toBeDefined();
      }
    }
  });

  test("items that ask have what they ask about; integrations are known", () => {
    for (const item of CATALOG.items) {
      if (item.ask === "obs_scene" || item.ask === "obs_input") expect(item.requires, item.id).toBe("obs");
      if (item.ask === "url" || item.ask === "keys" || item.ask === "obs_scene" || item.ask === "obs_input") expect(item.action, item.id).toBeDefined();
      if (item.widget || item.navigate || item.ask === "page" || item.ask === "sequence") expect(item.action, item.id).toBeUndefined();
    }
  });
});

describe("the shapes of the sets", () => {
  const cells = (set: (typeof SETS)[number]) => {
    const out: { id: string; x: number; y: number; w: number; h: number }[] = [];
    set.rows.forEach((row, y) => row.forEach((id, x) => id !== null && out.push({ id, x, y, w: catalogItem(id)?.widget?.w ?? 1, h: catalogItem(id)?.widget?.h ?? 1 })));
    return out;
  };

  test.each(SETS.map((s) => [s.id, s] as const))("%s: no two items overlap, and the shape is at most 4 wide and 7 high", (_id, set) => {
    const c = cells(set);
    for (let i = 0; i < c.length; i++) for (let j = i + 1; j < c.length; j++) {
      const a = c[i]!, b = c[j]!;
      expect(a.x < b.x + b.w && b.x < a.x + a.w && a.y < b.y + b.h && b.y < a.y + a.h, `${a.id} / ${b.id}`).toBe(false);
    }
    expect(Math.max(...c.map((x) => x.x + x.w))).toBeLessThanOrEqual(4);
    expect(Math.max(...c.map((x) => x.y + x.h))).toBeLessThanOrEqual(7);
  });

  test.each(SETS.map((s) => [s.id, s] as const))("%s: it goes whole onto an empty page of the default 7x4 and of the portrait 4x7", (_id, set) => {
    const ctx: InsertContext = { language: "en", scenes: ["Game", "Talk", "Wait"], inputs: ["Mic/Aux"], icons: { "ph:shuffle": "lucide:square", "ph:repeat": "lucide:square" } };
    for (const g of [grid(7, 4), grid(4, 7)]) {
      const layout: LayoutFile = { ...base, grid: g, display: { ...base.display, ...(g.cols === 4 ? { orientation: "portrait" as const } : {}) }, pages: [{ id: "main", name: "Main", items: [] }] };
      const result = insertSet(layout, actionsBase, "main", set, ctx);
      if (!result.ok) throw new Error(result.error);
      expect(result.placedOn, `${set.id} on ${g.cols}x${g.rows}`).toEqual(["main"]);
      expect(result.skipped).toEqual([]);
      expect(validateLayout(result.layout, result.actions), `${set.id} on ${g.cols}x${g.rows}`).toEqual([]);
      expect(validateActions(result.actions)).toEqual([]);
    }
  });
});

describe("previewSet (DESIGN.md 4.8)", () => {
  const media = SETS.find((s) => s.id === "media-remote")!;
  const empty: LayoutFile = { ...base, grid: grid(7, 4), pages: [{ id: "main", name: "Main", items: [] }] };
  const full: LayoutFile = {
    ...empty,
    pages: [{ id: "main", name: "Main", items: Array.from({ length: 28 }, (_, i) => ({ id: `k${i}`, kind: "button", x: i % 7, y: Math.floor(i / 7), w: 1, h: 1, actionId: "lock" }) as LayoutItem) }],
  };
  const ctx: InsertContext = { language: "en", icons: { "ph:shuffle": "lucide:square", "ph:repeat": "lucide:square" } };
  const taken = (l: LayoutFile) => l.pages[0]!.items.map((i) => `${i.x},${i.y},${i.w},${i.h}`).sort();
  const shadow = (cells: { x: number; y: number; w: number; h: number }[]) => cells.map((c) => `${c.x},${c.y},${c.w},${c.h}`).sort();

  test("it shows the cells insertSet takes, and changes nothing", () => {
    const before = JSON.stringify(empty);
    const p = previewSet(empty, "main", media)!;
    expect(p.newPage).toBeNull();
    const done = insertSet(empty, actionsBase, "main", media, ctx);
    if (!done.ok) throw new Error(done.error);
    expect(shadow(p.cells)).toEqual(taken(done.layout));
    expect(JSON.stringify(empty)).toBe(before);
  });

  test("dropped on a cell, the shape starts there when it fits, and insertSet agrees", () => {
    const at = { x: 3, y: 1 };
    const p = previewSet(empty, "main", media, at)!;
    expect(Math.min(...p.cells.map((c) => c.x))).toBe(3);
    expect(Math.min(...p.cells.map((c) => c.y))).toBe(1);
    const done = insertSet(empty, actionsBase, "main", media, { ...ctx, at });
    if (!done.ok) throw new Error(done.error);
    expect(shadow(p.cells)).toEqual(taken(done.layout));
  });

  test("a full page: the hint is the name of the new page", () => {
    const p = previewSet(full, "main", media, { x: 0, y: 0 })!;
    expect(p.newPage).toBe(media.name.en);
    const done = insertSet(full, actionsBase, "main", media, ctx);
    if (!done.ok) throw new Error(done.error);
    expect(done.placedOn).not.toEqual(["main"]);
    expect(done.layout.pages[1]!.name).toBe(p.newPage);
  });

  test("an unknown page gives nothing", () => {
    expect(previewSet(empty, "nope", media)).toBeNull();
  });
});

describe("insertSet (DATA_MODEL.md 9)", () => {
  const ctx: InsertContext = { language: "en", accent: "#123456", icons: { "ph:shuffle": "img:0123456789abcdef", "ph:repeat": "img:fedcba9876543210" }, random: seeded(7) };
  const windows = SETS.find((s) => s.id === "windows-tools")!;
  const stream = SETS.find((s) => s.id === "stream")!;
  const mediaRemote = SETS.find((s) => s.id === "media-remote")!;
  const empty = (items: LayoutItem[] = [], extra: Partial<LayoutFile> = {}): LayoutFile => ({ ...base, pages: [{ id: "main", name: "Main", items }, { id: "other", name: "Other", items: [] }], ...extra });
  const key = (x: number, y: number): LayoutItem => ({ id: `k-${x}-${y}`, kind: "button", x, y, w: 1, h: 1, actionId: "lock" }) as LayoutItem;
  const ok = (r: ReturnType<typeof insertSet>) => {
    if (!r.ok) throw new Error(r.error);
    return r;
  };

  test("the whole shape goes into an empty page, from the top left corner, and the result is valid", () => {
    const r = ok(insertSet(empty(), actionsBase, "main", windows, ctx));
    expect(r.placedOn).toEqual(["main"]);
    const items = r.layout.pages[0]!.items as { x: number; y: number; label?: string; actionId?: string }[];
    expect(items).toHaveLength(8);
    expect(items.map((i) => [i.x, i.y])).toEqual([[0, 0], [1, 0], [2, 0], [3, 0], [0, 1], [1, 1], [2, 1], [3, 1]]);
    expect(items.map((i) => i.label)).toEqual(["Lock", "Screenshot", "Task mgr", "Desktop", "Explorer", "Emoji", "Clipboard", "Settings"]);
    expect(validateLayout(r.layout, r.actions)).toEqual([]);
    expect(r.layout.pages[1]).toEqual(base.pages.length > 0 ? { id: "other", name: "Other", items: [] } : undefined);
  });

  test("the shape goes where it fits: past what is already on the page", () => {
    const busy = empty([key(0, 0), key(1, 0), key(2, 0), key(3, 0), key(4, 0), key(5, 0), key(6, 0)]);
    const r = ok(insertSet(busy, actionsBase, "main", windows, ctx));
    const added = (r.layout.pages[0]!.items as { id: string; y: number }[]).filter((i) => r.added.includes(i.id));
    expect(added.map((i) => i.y)).toEqual([1, 1, 1, 1, 2, 2, 2, 2]);
    expect(validateLayout(r.layout, r.actions)).toEqual([]);
  });

  test("a fragmented page: one by one into the free cells, in the order of the shape", () => {
    const checker: LayoutItem[] = [];
    for (let y = 0; y < 4; y++) for (let x = 0; x < 7; x++) if ((x + y) % 2 === 0) checker.push(key(x, y));
    const r = ok(insertSet(empty(checker), actionsBase, "main", windows, ctx));
    expect(r.placedOn).toEqual(["main"]);
    expect(r.layout.pages).toHaveLength(2); // no new page: 14 cells are free for 8 items
    const added = (r.layout.pages[0]!.items as { id: string; x: number; y: number; label: string }[]).filter((i) => r.added.includes(i.id));
    expect(added.map((i) => [i.x, i.y])).toEqual([[1, 0], [3, 0], [5, 0], [0, 1], [2, 1], [4, 1], [6, 1], [1, 2]]);
    expect(added.map((i) => i.label)).toEqual(["Lock", "Screenshot", "Task mgr", "Desktop", "Explorer", "Emoji", "Clipboard", "Settings"]);
    expect(validateLayout(r.layout, r.actions)).toEqual([]);
  });

  test("not enough free cells: a new page with the name of the set, right after this one", () => {
    const crowded: LayoutItem[] = [];
    for (let y = 0; y < 4; y++) for (let x = 0; x < 7; x++) if (y * 7 + x < 23) crowded.push(key(x, y)); // 5 cells are left
    const r = ok(insertSet(empty(crowded), actionsBase, "main", windows, ctx));
    expect(r.layout.pages.map((p) => p.name)).toEqual(["Main", "Windows tools", "Other"]);
    expect(r.placedOn).toEqual(["windows-tools"]);
    expect(r.layout.pages[0]!.items).toHaveLength(23); // the first page is untouched
    expect(r.layout.pages[1]!.items).toHaveLength(8);
    expect(validateLayout(r.layout, r.actions)).toEqual([]);
  });

  test("a page limit stops a new page", () => {
    const pages = Array.from({ length: 10 }, (_, i) => ({ id: `p${i}`, name: `P${i}`, items: i === 0 ? ([1, 2, 3, 4, 5, 6, 7].flatMap((n) => [0, 1, 2, 3].map((y) => key(n - 1, y))) as LayoutItem[]) : [] }));
    const r = insertSet({ ...base, pages }, actionsBase, "p0", windows, ctx);
    expect(r).toEqual({ ok: false, error: "Page limit reached" });
  });

  test("items that ask are skipped without an answer; OBS scenes are taken in order", () => {
    const none = ok(insertSet(empty(), actionsBase, "main", stream, ctx));
    expect(none.skipped.map((s) => [s.id, s.reason])).toEqual([["stream.mic", "no_answer"], ["stream.scene", "no_answer"], ["stream.scene", "no_answer"], ["stream.scene", "no_answer"]]);
    expect((none.layout.pages[0]!.items as { label?: string }[]).map((i) => i.label).filter(Boolean)).toEqual(["Record", "Go live", "Replay"]);

    const full = ok(insertSet(empty(), actionsBase, "main", stream, { ...ctx, scenes: ["Game", "Talk", "Wait", "Spare"], inputs: ["Mic/Aux"] }));
    expect(full.skipped).toEqual([]);
    const buttons = full.layout.pages[0]!.items.filter((i): i is Extract<LayoutItem, { kind: "button" }> => i.kind === "button");
    const scenes = buttons.filter((b) => b.actionId?.startsWith("obs-switch-scene")).map((b) => [b.label, Object.keys(b.bind!.states)]);
    expect(scenes).toEqual([["Scene", ["Game"]], ["Scene", ["Talk"]], ["Scene", ["Wait"]]]);
    expect(Object.entries(full.actions.actions).filter(([, a]) => a.type === "obs" && a.op === "scene").map(([, a]) => (a as { scene: string }).scene)).toEqual(["Game", "Talk", "Wait"]);
    expect(full.layout.pages[0]!.items.some((i) => i.kind === "obs")).toBe(true);
    expect(validateLayout(full.layout, full.actions)).toEqual([]);
  });

  test("a second insertion adds the keys again and reuses the actions", () => {
    const first = ok(insertSet(empty(), actionsBase, "main", mediaRemote, ctx));
    const second = ok(insertSet({ ...first.layout, pages: [{ ...first.layout.pages[0]! }, ...first.layout.pages.slice(1)] }, first.actions, "other", mediaRemote, ctx));
    expect(Object.keys(second.actions.actions)).toEqual(Object.keys(first.actions.actions));
    const all = second.layout.pages.flatMap((p) => p.items.map((i) => i.id));
    expect(new Set(all).size).toBe(all.length);
    expect(second.layout.pages[1]!.items).toHaveLength(first.layout.pages[0]!.items.length);
    expect(validateLayout(second.layout, second.actions)).toEqual([]);
  });

  test("the accent, the language and the icons of Phosphor are put in", () => {
    const r = ok(insertSet(empty(), actionsBase, "main", mediaRemote, { ...ctx, language: "ru" }));
    const keys = r.layout.pages[0]!.items.filter((i): i is Extract<LayoutItem, { kind: "button" }> => i.kind === "button");
    const shuffle = keys.find((k) => k.actionId === "shuffle")!;
    expect(shuffle).toMatchObject({ label: "Случайно", icon: "img:0123456789abcdef", bind: { var: "media.shuffle", states: { true: { color: "#123456" } } } });
    const repeat = keys.find((k) => k.actionId === "repeat")!;
    expect(repeat.bind!.states).toEqual({ list: { color: "#123456" }, track: { label: "Повтор 1", color: "#123456" } });
    // The action carries the name in the language of the window.
    expect(r.actions.actions["shuffle"]).toMatchObject({ name: "Случайный порядок", type: "media", op: "shuffle" });
    // The widget got the ids of the actions of the keys it needs.
    const widget = r.layout.pages[0]!.items.find((i) => i.kind === "media") as { prevAction: string; playAction: string; nextAction: string };
    // The default actions already do the same, so they are reused instead of added a second time.
    expect([widget.prevAction, widget.playAction, widget.nextAction]).toEqual(["media-prev", "media-play", "media-next"]);
  });

  test("an icon of Phosphor that was not converted becomes a square", () => {
    const r = ok(insertSet(empty(), actionsBase, "main", mediaRemote, { language: "en", random: seeded(3) }));
    const icons = r.layout.pages[0]!.items.map((i) => (i as { icon?: string }).icon).filter(Boolean);
    expect(icons.every((i) => !i!.startsWith("ph:"))).toBe(true);
    expect(validateLayout(r.layout, r.actions)).toEqual([]);
  });

  test("a set whose items are all unknown puts nothing and says so", () => {
    const r = ok(insertSet(empty(), actionsBase, "main", { ...windows, rows: [["nope.item"]] }, ctx));
    expect(r.skipped).toEqual([{ id: "nope.item", reason: "unknown" }]);
    expect(r.layout.pages[0]!.items).toEqual([]);
    expect(r.placedOn).toEqual([]);
  });

  test("an unknown page is an error", () => {
    expect(insertSet(empty(), actionsBase, "gone", windows, ctx)).toEqual({ ok: false, error: "no page 'gone'" });
  });

  test("a widget that does not fit anywhere is skipped and named in the result", () => {
    // A page where only 1x1 holes are left: the 3x1 Now playing widget finds no place, the keys do.
    const holes: LayoutItem[] = [];
    for (let y = 0; y < 4; y++) for (let x = 0; x < 7; x++) if ((x + y) % 2 === 0) holes.push(key(x, y));
    const r = ok(insertSet(empty(holes), actionsBase, "main", mediaRemote, ctx));
    expect(r.skipped).toEqual([{ id: "media.widget", reason: "no_room" }]);
    expect(r.added).toHaveLength(5);
  });
});

describe("insertItem", () => {
  const ctx: InsertContext = { language: "en", random: seeded(11) };
  const page = (items: LayoutItem[] = []): LayoutFile => ({ ...base, pages: [{ id: "main", name: "Main", items }] });

  test("a key goes to the first free cell, with its action", () => {
    const r = insertItem(page(), actionsBase, "main", "win.screenshot", {}, ctx);
    if (!r.ok) throw new Error(r.error);
    expect(r.layout.pages[0]!.items).toHaveLength(1);
    expect(r.layout.pages[0]!.items[0]).toMatchObject({ kind: "button", x: 0, y: 0, label: "Screenshot", icon: "lucide:camera" });
    expect(validateLayout(r.layout, r.actions)).toEqual([]);
  });

  test("an answered ask: a website, a shortcut, a page, a group", () => {
    let layout = { ...page(), pages: [{ id: "main", name: "Main", items: [] as LayoutItem[] }, { id: "dev", name: "Dev", items: [] as LayoutItem[] }, { id: "tools", name: "Tools", popup: true, items: [] as LayoutItem[] }] };
    let actions = actionsBase;
    for (const [item, answer] of [["web.site", { url: "https://github.com" }], ["keys.shortcut", { keys: ["ctrl", "alt", "m"] }], ["nav.page", { page: "dev" }], ["nav.popup", { page: "tools" }]] as const) {
      const r = insertItem(layout, actions, "main", item, answer as unknown as Answer, ctx);
      if (!r.ok) throw new Error(`${item}: ${r.error}`);
      layout = r.layout;
      actions = r.actions;
    }
    const keys = layout.pages[0]!.items as { navigate?: string; popup?: string; actionId?: string }[];
    expect(keys.map((k) => [k.actionId !== undefined, k.navigate, k.popup])).toEqual([[true, undefined, undefined], [true, undefined, undefined], [false, "dev", undefined], [false, undefined, "tools"]]);
    expect(Object.values(actions.actions).filter((a) => a.type === "open_url" || a.type === "hotkey").length).toBeGreaterThanOrEqual(2);
    expect(validateLayout(layout, actions)).toEqual([]);
  });

  test("an item without its answer puts nothing", () => {
    const r = insertItem(page(), actionsBase, "main", "web.site", {}, ctx);
    if (!r.ok) throw new Error(r.error);
    expect(r.skipped).toEqual([{ id: "web.site", reason: "no_answer" }]);
    expect(r.layout.pages[0]!.items).toEqual([]);
  });

  test("the back key needs no action", () => {
    const r = insertItem(page(), actionsBase, "main", "nav.back", {}, ctx);
    if (!r.ok) throw new Error(r.error);
    expect(r.layout.pages[0]!.items[0]).toMatchObject({ kind: "button", navigate: "$back", icon: "lucide:arrow-left", label: "Back" });
    expect(validateLayout(r.layout, r.actions)).toEqual([]);
  });

  test("a widget takes its own size; a full page is an error with the text of the window", () => {
    const r = insertItem(page(), actionsBase, "main", "widget.sysgraph", {}, ctx);
    if (!r.ok) throw new Error(r.error);
    expect(r.layout.pages[0]!.items[0]).toMatchObject({ kind: "sys", w: 3, h: 1, graph: true });
    const full: LayoutItem[] = [];
    for (let y = 0; y < 4; y++) for (let x = 0; x < 7; x++) full.push({ id: `f-${x}-${y}`, kind: "button", x, y, w: 1, h: 1, actionId: "lock" } as LayoutItem);
    expect(insertItem(page(full), actionsBase, "main", "win.lock", {}, ctx)).toEqual({ ok: false, error: "No free cell on this page" });
  });

  test("a chosen cell is used, or refused when it is taken", () => {
    const r = insertItem(page(), actionsBase, "main", "win.lock", {}, ctx, { x: 3, y: 2 });
    if (!r.ok) throw new Error(r.error);
    expect(r.layout.pages[0]!.items[0]).toMatchObject({ x: 3, y: 2, confirm: true, color: "#b91c1c" });
    expect(insertItem(r.layout, r.actions, "main", "win.sleep", {}, ctx, { x: 3, y: 2 })).toEqual({ ok: false, error: "No free cell on this page" });
  });

  test("an unknown item is an error", () => {
    expect(insertItem(page(), actionsBase, "main", "nope.item", {}, ctx)).toEqual({ ok: false, error: "unknown catalog item 'nope.item'" });
  });
});

describe("the library", () => {
  test("a search finds names and labels in both languages and whole categories", () => {
    expect(searchCatalog("screenshot").map((i) => i.id)).toEqual(["win.screenshot"]);
    expect(searchCatalog("Снимок").map((i) => i.id)).toContain("win.screenshot");
    expect(searchCatalog("повтор").map((i) => i.id)).toEqual(expect.arrayContaining(["media.repeat", "stream.replay"]));
    expect(searchCatalog("stream").map((i) => i.id)).toEqual(expect.arrayContaining(["stream.record", "stream.live", "stream.scene"]));
    expect(searchCatalog("  ")).toEqual([]);
    expect(searchCatalog("zzzzz")).toEqual([]);
  });

  test("categories list their items in order", () => {
    const media = CATALOG.categories.find((c) => c.id === "media")!;
    expect(categoryItems(media).map((i) => i.id)).toEqual(media.items);
  });
});

function seeded(seed: number): () => number {
  let s = seed;
  return () => {
    s = (s * 1664525 + 1013904223) % 4294967296;
    return s / 4294967296;
  };
}
