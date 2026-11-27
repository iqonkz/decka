import { t, type TranslationKey } from "@decka/shared";

export type SearchTargetPage =
  | "deck"
  | "look"
  | "phone"
  | "voice"
  | "screen_help"
  | "notes"
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
  {
    id: "screen_help", page: "screen_help", titleKey: "sidebar.item.screen_help", keywords: ["screen", "assist", "pack", "uia"],
    keyKeys: ["m28.screen_help_title", "m29.screen_pack_title"],
  },
  {
    id: "notes", page: "notes", titleKey: "sidebar.item.notes", keywords: ["notes", "meeting", "markdown", "audio"],
    keyKeys: ["m30.notes_open_folder"],
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

  // Settings inside pages: `anchor` is the id of the element on the page without its `section-` prefix (see `anchorId`).
  { id: "look-theme", page: "look", anchor: "look-theme", titleKey: "look.theme" },
  { id: "look-accent", page: "look", anchor: "look-accent", titleKey: "look.accent" },
  { id: "look-wallpaper", page: "look", anchor: "look-wallpaper", titleKey: "look.wallpaper" },
  { id: "look-night", page: "look", anchor: "look-night", titleKey: "look.night" },
  { id: "phone-pair", page: "phone", anchor: "phone-pairing", titleKey: "pair.title" },
  { id: "phone-saver", page: "look", anchor: "look-screen-saving", titleKey: "look.screen_saving" },
  { id: "voice-mic", page: "voice", anchor: "voice-mic", titleKey: "voice.mic" },
  { id: "voice-hotkey", page: "voice", anchor: "voice-hotkey", titleKey: "voice.hotkey" },
  { id: "voice-dictation", page: "voice", anchor: "voice-dictation", titleKey: "dict.title" },
  { id: "voice-memory", page: "voice", anchor: "voice-memory", titleKey: "mem.title" },
  { id: "integrations-mail", page: "integrations", anchor: "integrations-mail", titleKey: "integ.item_mail_name" },
  { id: "integrations-weather", page: "integrations", anchor: "integrations-weather", titleKey: "weather.title" },
  { id: "integrations-lyrics", page: "integrations", anchor: "integrations-lyrics", titleKey: "integ.item_lyrics_name", keywords: ["lyrics", "lrclib", "music"] },
  { id: "integrations-obs", page: "integrations", anchor: "integrations-obs", titleKey: "obsset.title" },
  { id: "integrations-twitch", page: "integrations", anchor: "integrations-twitch", titleKey: "twitchset.title" },
  { id: "integrations-youtube", page: "integrations", anchor: "integrations-youtube", titleKey: "youtubeset.title" },
  { id: "settings-lang", page: "settings", anchor: "settings-language", titleKey: "settings.language" },
  { id: "settings-net", page: "settings", anchor: "settings-port", titleKey: "settings.port" },
  { id: "settings-autostart", page: "settings", anchor: "settings-autostart", titleKey: "settings.autostart" },
  { id: "settings-usb", page: "settings", anchor: "settings-usb", titleKey: "settings.usb" },
  { id: "settings-backup", page: "settings", anchor: "settings-backup", titleKey: "backup.title", keywords: ["backup", "export", "import", "decka"] },
  { id: "about-updates", page: "about", anchor: "about-updates", titleKey: "about.updates" },
  { id: "about-report", page: "about", anchor: "about-report", titleKey: "about.report", keywords: ["report", "log", "diagnostics"] },
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
export function targetForPage(page: SearchTargetPage): { tab: "deck" | "look" | "phone" | "more"; section?: "actions" | "integrations" | "settings" | "voice" | "screen_help" | "notes" | "reminders" | "about" } {
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

/** The id of the element of a registry entry on its page. */
export function anchorId(entry: SearchEntry): string | null {
  return entry.anchor ? `section-${entry.anchor}` : null;
}

/** Scrolls the element of the entry into view and highlights it for 1.2 s (accent 10 %, DESIGN.md 4.14); false when the page has no such element. */
export function revealAnchor(entry: SearchEntry, root: Document = document, hold = 1200): boolean {
  const id = anchorId(entry);
  const el = id ? root.getElementById(id) : null;
  if (!el) return false;
  el.scrollIntoView({ behavior: "smooth", block: "center" });
  el.classList.add("search-highlight");
  setTimeout(() => el.classList.remove("search-highlight"), hold);
  return true;
}