import type { Language } from "./languages";

/**
 * A text of the data (catalog, sets, templates) in several languages (DATA_MODEL.md 9, M19). English is always there; a
 * language that has no text falls back to English.
 */
export type LocalizedText = { en: string } & Partial<Record<Language, string>>;

/** The text in `language`, else English. */
export function pickText(text: LocalizedText | undefined, language: Language): string {
  if (!text) return "";
  return text[language] ?? text.en;
}
