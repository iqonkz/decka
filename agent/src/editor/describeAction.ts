import { t, type Action } from "@decka/shared";
import { domainOf, formatChord } from "./addButton";

const MEDIA_KEYS = {
  play_pause: "preset.play_pause",
  next: "preset.next",
  prev: "preset.prev",
  vol_up: "preset.vol_up",
  vol_down: "preset.vol_down",
  mute: "preset.mute",
} as const;

/** The one-line answer to "what does this button do" (DESIGN.md 4.6, the `Does:` row): `Launch Spotify`, `Open github.com`... */
export function describeAction(action: Action | undefined): string {
  if (!action) return t("does.nothing");
  switch (action.type) {
    case "launch":
      return t("does.launch", { name: action.name });
    case "open_url":
      return t("does.open", { what: domainOf(action.url) ?? action.url });
    case "hotkey":
      return t("does.press", { keys: formatChord(action.keys) });
    case "media": {
      const key = MEDIA_KEYS[action.op as keyof typeof MEDIA_KEYS];
      return key ? t(key) : action.name;
    }
    case "system":
      return action.op === "sleep" ? t("preset.sleep") : t("preset.lock");
    case "screenshot":
      return t(`m29.shot_${action.area}`);
    case "http":
      return action.name;
    case "notes_record":
      return t("notes.action_type");
    case "ha":
      return action.name;
    case "audio":
    case "obs":
    case "twitch":
    case "youtube":
    case "sequence":
      return action.name;
  }
}
