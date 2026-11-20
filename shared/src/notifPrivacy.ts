import type { NotifPrivacy, PrivacyMode } from "./config";

/** Programs whose notifications carry other people's words: `app` unless the owner says otherwise (DATA_MODEL.md 11.5). Mirrors winnotif.rs. */
const PRIVATE_BY_DEFAULT = ["telegram", "whatsapp", "signal", "viber", "discord", "slack", "teams", "outlook", "mail", "thunderbird", "messenger", "skype"];

/** How much of a notification of `app` the agent sends: the owner's choice for the program, else the built-in messenger list, else the owner's default. */
export function effectivePrivacy(app: string, privacy: NotifPrivacy | undefined): PrivacyMode {
  const own = Object.entries(privacy?.apps ?? {}).find(([name]) => name.toLowerCase() === app.toLowerCase());
  if (own) return own[1];
  const lower = app.toLowerCase();
  if (PRIVATE_BY_DEFAULT.some((word) => lower.includes(word))) return "app";
  return privacy?.default ?? "full";
}
