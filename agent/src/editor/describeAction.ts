import type { Action } from "@decka/shared";
import { domainOf, formatChord } from "./addButton";

const MEDIA: Record<string, string> = {
  play_pause: "Play / Pause",
  next: "Next track",
  prev: "Previous track",
  vol_up: "Volume up",
  vol_down: "Volume down",
  mute: "Mute",
};

/** The one-line answer to "what does this button do" (DESIGN.md 4.6, the `Does:` row): `Launch Spotify`, `Open github.com`... */
export function describeAction(action: Action | undefined): string {
  if (!action) return "Nothing yet";
  switch (action.type) {
    case "launch":
      return `Launch ${action.name}`;
    case "open_url":
      return `Open ${domainOf(action.url) ?? action.url}`;
    case "hotkey":
      return `Press ${formatChord(action.keys)}`;
    case "media":
      return MEDIA[action.op] ?? action.name;
    case "system":
      return action.op === "sleep" ? "Sleep" : "Lock PC";
  }
}