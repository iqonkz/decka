import type { Action, ActionsFile, InstalledApp, LayoutFile, LayoutItem } from "@decka/shared";
import { fits, launchActionOf, makeItem, withMediaDefaults, type Kind } from "../tabs/layoutOps";

/** A ready-made action of the quick picker (DESIGN.md 4.5, table `Media & system`). */
export interface Preset {
  id: string;
  label: string;
  action: Action;
  icon: string;
  confirm?: boolean;
}

/** `Media & system` (DESIGN.md 4.5). */
export function presetActions(): Preset[] {
  return [
    { id: "play-pause", label: "Play / Pause", action: { name: "Play / Pause", type: "media", op: "play_pause" }, icon: "lucide:play" },
    { id: "next-track", label: "Next track", action: { name: "Next track", type: "media", op: "next" }, icon: "lucide:skip-forward" },
    { id: "prev-track", label: "Previous track", action: { name: "Previous track", type: "media", op: "prev" }, icon: "lucide:skip-back" },
    { id: "volume-up", label: "Volume up", action: { name: "Volume up", type: "media", op: "vol_up" }, icon: "lucide:volume-2" },
    { id: "volume-down", label: "Volume down", action: { name: "Volume down", type: "media", op: "vol_down" }, icon: "lucide:volume-1" },
    { id: "mute", label: "Mute", action: { name: "Mute", type: "media", op: "mute" }, icon: "lucide:volume-x" },
    { id: "lock-pc", label: "Lock PC", action: { name: "Lock PC", type: "system", op: "lock" }, icon: "lucide:lock", confirm: true },
    { id: "sleep", label: "Sleep", action: { name: "Sleep", type: "system", op: "sleep" }, icon: "lucide:moon", confirm: true },
  ];
}

const FALLBACK_APP_ICON = "lucide:rocket";
const MAX_LABEL = 24;

/** What the user picked in the quick picker. */
export type Choice =
  | { type: "app"; app: InstalledApp; /** `img:` icon of the program, when one could be made. */ icon?: string }
  | { type: "preset"; presetId: string }
  | { type: "action"; actionId: string }
  | { type: "widget"; kind: Kind }
  | { type: "url"; url: string }
  | { type: "keys"; keys: string[] };

/** `launch` action of a program (program, arguments and working folder come from the shortcut). */
export const actionFromApp = (app: InstalledApp): Action => launchActionOf(app);

/** The domain of a site, without `www.`; the label of a website button. */
export function domainOf(url: string): string | null {
  const parsed = parseWebUrl(url);
  return parsed ? parsed.hostname.replace(/^www\./i, "") : null;
}

/** `github.com/x` is taken as `https://github.com/x`; only http and https are accepted (DATA_MODEL.md 2.3). */
function parseWebUrl(input: string): URL | null {
  const text = input.trim();
  if (text === "" || text.length > 2000) return null;
  const withScheme = /^[a-z][a-z0-9+.-]*:\/\//i.test(text) ? text : `https://${text}`;
  try {
    const url = new URL(withScheme);
    return (url.protocol === "http:" || url.protocol === "https:") && url.hostname.includes(".") ? url : null;
  } catch {
    return null;
  }
}

/** `open_url` action for a website; `null` when the text is not a web address. */
export function actionFromUrl(input: string): Action | null {
  const url = parseWebUrl(input);
  if (!url) return null;
  const domain = url.hostname.replace(/^www\./i, "");
  return { name: domain.slice(0, 60), type: "open_url", url: url.href };
}

const KEY_LABELS: Record<string, string> = { ctrl: "Ctrl", alt: "Alt", shift: "Shift", win: "Win", pageup: "PageUp", pagedown: "PageDown" };

/** `["ctrl","shift","m"]` -> `Ctrl+Shift+M`. */
export function formatChord(keys: string[]): string {
  return keys.map((k) => KEY_LABELS[k] ?? (k.length === 1 ? k.toUpperCase() : /^f\d+$/.test(k) ? k.toUpperCase() : k[0]!.toUpperCase() + k.slice(1))).join("+");
}

/** `hotkey` action for a chord; `null` for no keys. */
export function actionFromKeys(keys: string[]): Action | null {
  if (keys.length < 1 || keys.length > 5) return null;
  return { name: formatChord(keys), type: "hotkey", keys };
}

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

/** The icon a button for `action` gets when nothing better is known (DESIGN.md 4.5). */
export function defaultIconFor(action: Action): string {
  switch (action.type) {
    case "hotkey":
      return "lucide:zap";
    case "open_url":
      return "lucide:globe";
    case "launch":
      return FALLBACK_APP_ICON;
    case "system":
      return action.op === "sleep" ? "lucide:moon" : "lucide:lock";
    case "media":
      return presetActions().find((p) => p.action.type === "media" && p.action.op === action.op)?.icon ?? "lucide:play";
  }
}

export type AddResult = { ok: true; layout: LayoutFile; actions: ActionsFile; itemId: string; actionsChanged: boolean } | { ok: false; error: string };

/**
 * Puts the chosen thing on cell (x, y) of page `pageIndex`: a button with its name, icon and action filled in, or a widget.
 * An action with the same content that already exists is reused; otherwise a new one is added. The cell must be free
 * for the item's size, else the result is an error and nothing changes.
 */
export function addButtonAt(layout: LayoutFile, actions: ActionsFile, pageIndex: number, cell: { x: number; y: number }, choice: Choice): AddResult {
  const page = layout.pages[pageIndex];
  if (!page) return { ok: false, error: "No such page" };

  let item: LayoutItem;
  let action: Action | null | undefined;
  let actionId: string | undefined;
  let label = "";
  let icon = "";
  let confirm = false;

  switch (choice.type) {
    case "widget":
      item = withMediaDefaults(makeItem(choice.kind, cell.x, cell.y), Object.keys(actions.actions));
      break;
    case "app":
      action = actionFromApp(choice.app);
      label = choice.app.name;
      icon = choice.icon ?? FALLBACK_APP_ICON;
      break;
    case "preset": {
      const preset = presetActions().find((p) => p.id === choice.presetId);
      if (!preset) return { ok: false, error: "Unknown ready-made action" };
      action = preset.action;
      label = preset.label;
      icon = preset.icon;
      confirm = preset.confirm === true;
      break;
    }
    case "action": {
      const existing = actions.actions[choice.actionId];
      if (!existing) return { ok: false, error: "Unknown action" };
      actionId = choice.actionId;
      label = existing.name;
      icon = defaultIconFor(existing);
      break;
    }
    case "url": {
      action = actionFromUrl(choice.url);
      if (!action) return { ok: false, error: "This is not a web address (http or https)" };
      label = domainOf(choice.url) ?? action.name;
      icon = "lucide:globe";
      break;
    }
    case "keys": {
      action = actionFromKeys(choice.keys);
      if (!action) return { ok: false, error: "Press at least one key" };
      label = formatChord(choice.keys);
      icon = "lucide:zap";
      break;
    }
  }

  let nextActions = actions;
  let actionsChanged = false;
  if (choice.type !== "widget") {
    if (action) {
      actionId = findSameAction(actions, action);
      if (actionId === undefined) {
        actionId = makeActionId(action.name, actions.actions);
        nextActions = { ...actions, actions: { ...actions.actions, [actionId]: action } };
        actionsChanged = true;
      }
    }
    item = { ...makeItem("button", cell.x, cell.y), label: label.slice(0, MAX_LABEL), icon, actionId: actionId!, ...(confirm ? { confirm: true } : {}) } as LayoutItem;
  } else {
    item = item!;
  }

  if (!fits(page, layout.grid, { x: item.x, y: item.y, w: item.w, h: item.h })) return { ok: false, error: "Not enough free space here" };
  const nextLayout: LayoutFile = { ...layout, pages: layout.pages.map((p, i) => (i === pageIndex ? { ...p, items: [...p.items, item] } : p)) };
  return { ok: true, layout: nextLayout, actions: nextActions, itemId: item.id, actionsChanged };
}
