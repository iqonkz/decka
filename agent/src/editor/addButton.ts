import { findSameAction, makeActionId, t, type Action, type ActionsFile, type InstalledApp, type LayoutFile, type LayoutItem } from "@decka/shared";
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
  // The labels are in the language of the window now; once a button is made they are the user's own text.
  const label = {
    "play-pause": t("preset.play_pause"),
    "next-track": t("preset.next"),
    "prev-track": t("preset.prev"),
    "volume-up": t("preset.vol_up"),
    "volume-down": t("preset.vol_down"),
    mute: t("preset.mute"),
    "lock-pc": t("preset.lock"),
    sleep: t("preset.sleep"),
  };
  return [
    { id: "play-pause", label: label["play-pause"], action: { name: label["play-pause"], type: "media", op: "play_pause" }, icon: "lucide:play" },
    { id: "next-track", label: label["next-track"], action: { name: label["next-track"], type: "media", op: "next" }, icon: "lucide:skip-forward" },
    { id: "prev-track", label: label["prev-track"], action: { name: label["prev-track"], type: "media", op: "prev" }, icon: "lucide:skip-back" },
    { id: "volume-up", label: label["volume-up"], action: { name: label["volume-up"], type: "media", op: "vol_up" }, icon: "lucide:volume-2" },
    { id: "volume-down", label: label["volume-down"], action: { name: label["volume-down"], type: "media", op: "vol_down" }, icon: "lucide:volume-1" },
    { id: "mute", label: label.mute, action: { name: label.mute, type: "media", op: "mute" }, icon: "lucide:volume-x" },
    { id: "lock-pc", label: label["lock-pc"], action: { name: label["lock-pc"], type: "system", op: "lock" }, icon: "lucide:lock", confirm: true },
    { id: "sleep", label: label.sleep, action: { name: label.sleep, type: "system", op: "sleep" }, icon: "lucide:moon", confirm: true },
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

// The helpers live in `@decka/shared` (insertSet uses them too); the editor keeps importing them from here.
export { findSameAction, makeActionId };

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
    case "screenshot":
      return "lucide:camera";
    case "http":
      return "lucide:link";
    case "media":
      return presetActions().find((p) => p.action.type === "media" && p.action.op === action.op)?.icon ?? "lucide:play";
    case "audio":
      return action.op === "mic_toggle" ? "lucide:mic-off" : "lucide:volume-x";
    case "obs":
      return "lucide:video";
    case "twitch":
      return action.op === "clip" ? "lucide:star" : "lucide:message-square";
    case "youtube":
      return action.op === "chat" ? "lucide:message-square" : action.op === "start" ? "lucide:video" : "lucide:video-off";
    case "sequence":
      return "lucide:zap";
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
  if (!page) return { ok: false, error: t("add.no_page") };

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
      if (!preset) return { ok: false, error: t("add.unknown_preset") };
      action = preset.action;
      label = preset.label;
      icon = preset.icon;
      confirm = preset.confirm === true;
      break;
    }
    case "action": {
      const existing = actions.actions[choice.actionId];
      if (!existing) return { ok: false, error: t("add.unknown_action") };
      actionId = choice.actionId;
      label = existing.name;
      icon = defaultIconFor(existing);
      break;
    }
    case "url": {
      action = actionFromUrl(choice.url);
      if (!action) return { ok: false, error: t("add.not_url") };
      label = domainOf(choice.url) ?? action.name;
      icon = "lucide:globe";
      break;
    }
    case "keys": {
      action = actionFromKeys(choice.keys);
      if (!action) return { ok: false, error: t("add.need_key") };
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

  if (!fits(page, layout.grid, { x: item.x, y: item.y, w: item.w, h: item.h })) return { ok: false, error: t("add.no_space") };
  const nextLayout: LayoutFile = { ...layout, pages: layout.pages.map((p, i) => (i === pageIndex ? { ...p, items: [...p.items, item] } : p)) };
  return { ok: true, layout: nextLayout, actions: nextActions, itemId: item.id, actionsChanged };
}
