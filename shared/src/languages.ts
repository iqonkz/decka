/** The languages of the interface (DESIGN.md 6, M19). English is the source of the strings. */
export const LANGUAGES = ["en", "zh", "hi", "es", "ar", "fr", "pt", "ru", "de", "ja", "kk"] as const;
export type Language = (typeof LANGUAGES)[number];

/** The name of each language in itself: the list of the window and of the wizard shows these (DESIGN.md 6). */
export const LANGUAGE_NAMES: Record<Language, string> = {
  en: "English",
  zh: "中文（简体）",
  hi: "हिन्दी",
  es: "Español",
  ar: "العربية",
  fr: "Français",
  pt: "Português (Brasil)",
  ru: "Русский",
  de: "Deutsch",
  ja: "日本語",
  kk: "Қазақша",
};

export function isLanguage(value: unknown): value is Language {
  return typeof value === "string" && (LANGUAGES as readonly string[]).includes(value);
}

/** Languages written from right to left: the window and the menus mirror, the grid of keys does not. */
export function isRtl(language: Language): boolean {
  return language === "ar";
}

/**
 * The order of the list of languages: the language of the system and English first, then the others by their own name
 * (`Intl.Collator` of the language of the system).
 */
export function languageOrder(system: Language): Language[] {
  const rest = LANGUAGES.filter((l) => l !== "en" && l !== system).sort((a, b) => LANGUAGE_NAMES[a].localeCompare(LANGUAGE_NAMES[b], system));
  return [...new Set<Language>([system, "en", ...rest])];
}
