import { CATALOG, getLanguage, makeActionId, pickText, t, type ButtonItem, type ButtonLook, type VarValue } from "@decka/shared";

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
 * The catalog item a button came from, by the id of its action: the catalog names a new action after the English name of
 * the item (`mute-microphone`, then `mute-microphone-2`); the built-in templates and the first layout use shorter ids.
 */
function catalogItemOf(actionId: string) {
  const base = actionId.replace(/-\d+$/, "");
  const alias = CATALOG_ID_ALIASES[actionId] ?? CATALOG_ID_ALIASES[base];
  if (alias) return CATALOG.items.find((c) => c.id === alias);
  return CATALOG.items.find((c) => {
    const id = makeActionId(c.name.en, {});
    return id === actionId || id === base;
  });
}

const same = (a: string, b: string) => a.trim().toLowerCase() === b.trim().toLowerCase();

/**
 * A caption the catalog wrote, in the language of the phone: the caption of the item or of one of its states ("Mic off"),
 * in whatever language it was written. Any other caption is the user's own and stays as it is.
 */
export function localizedButtonLabel(item: Pick<ButtonItem, "actionId">, label: string | undefined): string | undefined {
  // A key without a caption stays without one: that is the user's choice too.
  if (!item.actionId || !label) return label;
  const cat = catalogItemOf(item.actionId);
  if (!cat) return label;
  // Captions the templates wrote that differ from the catalog's.
  if ((cat.id === "win.taskmgr" && same(label, "Tasks")) || (cat.id === "media.play" && same(label, "Play / Pause"))) return pickText(cat.label, getLanguage());
  const captions = [cat.label, ...Object.values(cat.bind?.states ?? {}).map((state) => state.label)];
  const written = captions.find((caption) => caption !== undefined && Object.values(caption).some((text) => typeof text === "string" && same(text, label)));
  return written ? pickText(written, getLanguage()) : label;
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
