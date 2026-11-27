import { CATALOG, getLanguage, pickText, t, type ButtonItem, type ButtonLook, type VarValue } from "@decka/shared";

const CATALOG_ID_ALIASES: Record<string, string> = {
  "lock-pc": "win.lock",
  "sleep": "win.sleep",
  "shutdown": "win.shutdown",
  "restart": "win.restart",
  "signout": "win.signout",
  "hibernate": "win.hibernate",
  "displayoff": "win.displayoff",
  "recycle": "win.recycle",
  "screenshot": "win.screenshot",
  "screenshot-window": "win.screenshot.window",
  "screenshot-region": "win.screenshot.region",
  "taskmgr": "win.taskmgr",
  "desktop": "win.desktop",
  "explorer": "win.explorer",
  "emoji": "win.emoji",
  "clipboard": "win.clipboard",
  "settings": "win.settings",
  "media-play": "media.play",
  "play-pause": "media.play",
  "media-next": "media.next",
  "next-track": "media.next",
  "media-prev": "media.prev",
  "prev-track": "media.prev",
  "media-shuffle": "media.shuffle",
  "media-repeat": "media.repeat",
  "mic": "sound.mic",
  "mic-mute": "sound.mic",
  "vol-mute": "sound.mute",
  "mute": "sound.mute",
  "vol-up": "sound.volup",
  "volume-up": "sound.volup",
  "vol-down": "sound.voldown",
  "volume-down": "sound.voldown",
  "obs-rec": "stream.record",
  "obs-live": "stream.live",
  "obs-replay": "stream.replay",
  "twitch-clip": "twitch.clip",
  "note": "sound.note",
  "notes-record": "sound.note",
};

/**
 * Resolve localized catalog label on the phone by `actionId` or catalog prefix,
 * keeping user-customized button captions intact.
 */
export function localizedButtonLabel(item: Pick<ButtonItem, "actionId">, label: string | undefined): string | undefined {
  // A key without a caption stays without one: that is the user's choice too.
  if (!item.actionId || !label) return label;
  const rawId = item.actionId;
  const direct = CATALOG_ID_ALIASES[rawId] ?? rawId;
  const cat =
    CATALOG.items.find((c) => c.id === direct) ??
    CATALOG.items.find((c) => rawId.startsWith(`${c.id}.`) || rawId.startsWith(`${c.id}-`));
  if (!cat) return label;
  const normalized = label.trim().toLowerCase();
  const knownDefaults = new Set(
    Object.values(cat.label)
      .filter((s): s is string => typeof s === "string")
      .map((s) => s.trim().toLowerCase()),
  );
  // Also recognize common baked preset labels ("Tasks" -> win.taskmgr, "Play / Pause", etc.).
  if (cat.id === "win.taskmgr") knownDefaults.add("tasks");
  if (cat.id === "media.play") knownDefaults.add("play / pause");
  return knownDefaults.has(normalized) ? pickText(cat.label, getLanguage()) : label;
}

/** Base look of the button with `bind.states[String(var)]` laid over it (DATA_MODEL.md 3.4). */
export function resolveLook(item: ButtonItem, vars: Record<string, VarValue> | undefined): ButtonLook {
  const base: ButtonLook = {};
  if (item.label !== undefined) base.label = item.label;
  if (item.icon !== undefined) base.icon = item.icon;
  if (item.color !== undefined) base.color = item.color;
  if (!item.bind) return base;

  const value = vars?.[item.bind.var];
  const key = value === undefined ? "false" : String(value);
  const override = item.bind.states[key];
  return override ? { ...base, ...definedOnly(override) } : base;
}

function definedOnly(look: ButtonLook): ButtonLook {
  const out: ButtonLook = {};
  if (look.label !== undefined) out.label = look.label;
  if (look.icon !== undefined) out.icon = look.icon;
  if (look.color !== undefined) out.color = look.color;
  return out;
}

/** Toast title for a failed press, by error code. */
export function pressErrorTitle(e: { code: string; message?: string }): string {
  switch (e.code) {
    case "UNKNOWN_ACTION":
      return t("press.unknown_action");
    case "ACTION_FAILED": {
      // T29.P: a webhook says its status code (also as a step of a sequence), or that nothing answered.
      const status = /^(?:Step \d+: )?HTTP (\d{3})$/.exec(e.message ?? "")?.[1];
      if (status !== undefined) return t("press.webhook_code", { code: status });
      if (e.message?.endsWith("The webhook did not answer")) return t("press.webhook_silent");
      return t("press.failed");
    }
    case "RATE_LIMITED":
      return t("press.rate_limited");
    case "TIMEOUT":
      return t("press.timeout");
    case "OFFLINE":
      return t("press.offline");
    default:
      return e.message || e.code;
  }
}
