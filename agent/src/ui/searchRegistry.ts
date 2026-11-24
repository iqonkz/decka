import { t, type TranslationKey } from "@decka/shared";

export type SearchTargetPage =
  | "deck"
  | "look"
  | "phone"
  | "voice"
  | "reminders"
  | "integrations"
  | "actions"
  | "settings"
  | "about";

export interface SearchEntry {
  id: string;
  page: SearchTargetPage;
  anchor?: string;
  titleKey: TranslationKey;
  keywords?: string[];
}

export const SEARCH_REGISTRY: SearchEntry[] = [
  // Primary pages
  { id: "keys", page: "deck", titleKey: "sidebar.item.keys", keywords: ["buttons", "layout", "editor", "canvas"] },
  { id: "look", page: "look", titleKey: "sidebar.item.look", keywords: ["appearance", "theme", "accent", "grid", "style"] },
  { id: "phone", page: "phone", titleKey: "sidebar.item.phone", keywords: ["device", "mobile", "pairing", "qr", "screen saving"] },
  { id: "voice", page: "voice", titleKey: "sidebar.item.voice", keywords: ["assistant", "speech", "whisper", "qwen", "hotkey"] },
  { id: "reminders", page: "reminders", titleKey: "sidebar.item.reminders", keywords: ["lists", "tasks", "schedule"] },
  { id: "integrations", page: "integrations", titleKey: "sidebar.item.integrations", keywords: ["obs", "twitch", "youtube", "mail", "weather"] },
  { id: "actions", page: "actions", titleKey: "sidebar.item.actions", keywords: ["hotkey", "macro", "sequence", "launch", "command"] },
  { id: "settings", page: "settings", titleKey: "sidebar.item.settings", keywords: ["language", "network", "port", "usb", "system"] },
  { id: "about", page: "about", titleKey: "sidebar.item.about", keywords: ["version", "updates", "changelog", "licence"] },

  // Sections and settings inside pages
  { id: "look-theme", page: "look", anchor: "section-theme", titleKey: "look.theme" },
  { id: "look-accent", page: "look", anchor: "section-accent", titleKey: "look.accent" },
  { id: "phone-pair", page: "phone", anchor: "section-pairing", titleKey: "pair.title" },
  { id: "phone-saver", page: "phone", anchor: "section-screen-saving", titleKey: "look.screen_saving" },
  { id: "voice-dictation", page: "voice", anchor: "section-dictation", titleKey: "dict.title" },
  { id: "voice-memory", page: "voice", anchor: "section-memory", titleKey: "mem.title" },
  { id: "settings-lang", page: "settings", anchor: "section-language", titleKey: "settings.language" },
  { id: "settings-net", page: "settings", anchor: "section-network", titleKey: "settings.port" },
  { id: "about-updates", page: "about", anchor: "section-updates", titleKey: "about.updates" },
];

/** Search registry entries matching a query against localized title, keywords and id. */
export function searchRegistry(
  query: string,
  entries: SearchEntry[] = SEARCH_REGISTRY,
  translate: (key: TranslationKey) => string = t,
): SearchEntry[] {
  const q = query.trim().toLowerCase();
  if (!q) return [];

  const results: SearchEntry[] = [];
  for (const entry of entries) {
    let title = "";
    try {
      title = translate(entry.titleKey).toLowerCase();
    } catch {
      title = String(entry.titleKey).toLowerCase();
    }

    const matchesTitle = title.includes(q);
    const matchesKeywords = entry.keywords?.some((k) => k.toLowerCase().includes(q));
    const matchesId = entry.id.toLowerCase().includes(q);

    if (matchesTitle || matchesKeywords || matchesId) {
      results.push(entry);
    }
  }

  return results.slice(0, 8);
}

/** Map target search page to internal tab and optional more section. */
export function targetForPage(page: SearchTargetPage): { tab: "deck" | "look" | "phone" | "more"; section?: "actions" | "integrations" | "settings" | "voice" | "reminders" | "about" } {
  switch (page) {
    case "deck":
      return { tab: "deck" };
    case "look":
      return { tab: "look" };
    case "phone":
      return { tab: "phone" };
    default:
      return { tab: "more", section: page };
  }
}
