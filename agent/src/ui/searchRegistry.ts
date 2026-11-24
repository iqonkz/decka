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
  /** English words that find the entry whatever the language of the window is. */
  keywords?: string[];
  /** Titles of the settings that live on the page: translated like every other string, so a search in the language of the window finds them (11 languages). */
  keyKeys?: TranslationKey[];
}

export const SEARCH_REGISTRY: SearchEntry[] = [
  // Primary pages: every page lists the settings that are on it.
  { id: "keys", page: "deck", titleKey: "sidebar.item.keys", keywords: ["buttons", "layout", "editor", "canvas"] },
  {
    id: "look", page: "look", titleKey: "sidebar.item.look", keywords: ["appearance", "grid", "style"],
    keyKeys: ["look.theme", "look.accent", "look.wallpaper", "look.key_style", "look.show_names", "look.pages", "look.brightness", "look.night"],
  },
  {
    id: "phone", page: "phone", titleKey: "sidebar.item.phone", keywords: ["device", "mobile", "qr"],
    keyKeys: ["pair.title", "look.screen_saving", "look.dim_after", "look.saver_after"],
  },
  {
    id: "voice", page: "voice", titleKey: "sidebar.item.voice", keywords: ["assistant", "speech", "whisper", "qwen", "microphone"],
    keyKeys: ["voice.mic", "voice.hotkey", "dict.title", "mem.title"],
  },
  { id: "reminders", page: "reminders", titleKey: "sidebar.item.reminders", keywords: ["tasks", "schedule"], keyKeys: ["rem.tab_reminders", "rem.tab_lists"] },
  {
    id: "integrations", page: "integrations", titleKey: "sidebar.item.integrations", keywords: ["obs", "twitch", "youtube", "mail", "weather"],
    keyKeys: ["integ.item_mail_name", "weather.title", "obsset.title", "twitchset.title", "youtubeset.title", "integ.item_ai_name"],
  },
  { id: "actions", page: "actions", titleKey: "sidebar.item.actions", keywords: ["hotkey", "macro", "sequence", "launch", "command"] },
  {
    id: "settings", page: "settings", titleKey: "sidebar.item.settings", keywords: ["network", "system"],
    keyKeys: ["settings.language", "settings.port", "settings.autostart", "settings.usb", "settings.pc_name", "settings.who_connects"],
  },
  { id: "about", page: "about", titleKey: "sidebar.item.about", keywords: ["version", "changelog"], keyKeys: ["about.updates", "about.licence"] },

  // Settings inside pages (the anchors are optional: a page without one is opened at its top).
  { id: "look-theme", page: "look", anchor: "section-theme", titleKey: "look.theme" },
  { id: "look-accent", page: "look", anchor: "section-accent", titleKey: "look.accent" },
  { id: "look-wallpaper", page: "look", titleKey: "look.wallpaper" },
  { id: "look-night", page: "look", titleKey: "look.night" },
  { id: "phone-pair", page: "phone", anchor: "section-pairing", titleKey: "pair.title" },
  { id: "phone-saver", page: "phone", anchor: "section-screen-saving", titleKey: "look.screen_saving" },
  { id: "voice-mic", page: "voice", titleKey: "voice.mic" },
  { id: "voice-hotkey", page: "voice", titleKey: "voice.hotkey" },
  { id: "voice-dictation", page: "voice", anchor: "section-dictation", titleKey: "dict.title" },
  { id: "voice-memory", page: "voice", anchor: "section-memory", titleKey: "mem.title" },
  { id: "integrations-mail", page: "integrations", titleKey: "integ.item_mail_name" },
  { id: "integrations-weather", page: "integrations", titleKey: "weather.title" },
  { id: "integrations-obs", page: "integrations", titleKey: "obsset.title" },
  { id: "integrations-twitch", page: "integrations", titleKey: "twitchset.title" },
  { id: "integrations-youtube", page: "integrations", titleKey: "youtubeset.title" },
  { id: "settings-lang", page: "settings", anchor: "section-language", titleKey: "settings.language" },
  { id: "settings-net", page: "settings", anchor: "section-network", titleKey: "settings.port" },
  { id: "settings-autostart", page: "settings", titleKey: "settings.autostart" },
  { id: "settings-usb", page: "settings", titleKey: "settings.usb" },
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

  const text = (key: TranslationKey): string => {
    try {
      return translate(key).toLowerCase();
    } catch {
      return String(key).toLowerCase();
    }
  };

  // Exact matches of a title first, then the rest in registry order; pages come before the settings inside them.
  const scored: { entry: SearchEntry; rank: number; order: number }[] = [];
  entries.forEach((entry, order) => {
    const title = text(entry.titleKey);
    const inTitle = title.includes(q);
    const inKeywords = entry.keywords?.some((k) => k.toLowerCase().includes(q)) ?? false;
    const inSettings = entry.keyKeys?.some((k) => text(k).includes(q)) ?? false;
    const inId = entry.id.toLowerCase().includes(q);
    if (inTitle || inKeywords || inSettings || inId) {
      scored.push({ entry, rank: inTitle ? (title === q ? 0 : 1) : 2, order });
    }
  });
  scored.sort((a, b) => a.rank - b.rank || a.order - b.order);
  return scored.map((s) => s.entry).slice(0, 8);
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
