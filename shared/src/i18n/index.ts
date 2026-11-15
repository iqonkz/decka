import { en } from "./en";
import { ru } from "./ru";
import type { LanguageSetting } from "../layout";

/** The languages of the interface (DESIGN.md 6). English is the source of the strings. */
export type Language = "en" | "ru";

export type MessageKey = keyof typeof en;
type PluralCategory = "one" | "few" | "many" | "other";
/** `keys_count` for the plural family `keys_count.one`, `keys_count.other`, ... */
export type PluralBase<K extends string = MessageKey> = K extends `${infer B}.${PluralCategory}` ? B : never;
/** A key `t()` accepts: a plain message or the base name of a plural family (then `params.count` picks the form). */
export type TranslationKey = Exclude<MessageKey, `${string}.${PluralCategory}`> | PluralBase;
export type Params = Record<string, string | number>;

const tables: Record<Language, Record<string, string>> = { en, ru };

/** Uppercase `ru`-prefixed languages mean Russian; anything else is English. */
export function resolveLanguage(setting: LanguageSetting | string | undefined, systemLanguage: string | undefined): Language {
  if (setting === "en" || setting === "ru") return setting;
  return (systemLanguage ?? "").toLowerCase().startsWith("ru") ? "ru" : "en";
}

function pluralRules(language: Language): Intl.PluralRules {
  return new Intl.PluralRules(language === "ru" ? "ru" : "en");
}

function fill(template: string, params: Params | undefined): string {
  if (!params) return template;
  return template.replace(/\{(\w+)\}/g, (whole, name: string) => (name in params ? String(params[name]) : whole));
}

/** The text of `key` in `language`; a plural family is chosen by `params.count`. A missing text falls back to English, then to the key. */
export function translate(language: Language, key: TranslationKey, params?: Params): string {
  const table = tables[language];
  let found: string | undefined;
  if (typeof params?.count === "number") {
    const category = pluralRules(language).select(params.count);
    found = table[`${key}.${category}`] ?? table[`${key}.other`];
  }
  found ??= table[key] ?? tables.en[key] ?? tables.en[`${key}.other`] ?? key;
  return fill(found, params);
}

let current: Language = "en";
const listeners = new Set<() => void>();

export function getLanguage(): Language {
  return current;
}

/** Switches the language of `t()` and tells the subscribers (the roots of the apps re-render). */
export function setLanguage(language: Language): void {
  if (language === current) return;
  current = language;
  for (const l of Array.from(listeners)) l();
}

export function subscribeLanguage(listener: () => void): () => void {
  listeners.add(listener);
  return () => listeners.delete(listener);
}

/** The text of `key` in the current language. */
export function t(key: TranslationKey, params?: Params): string {
  return translate(current, key, params);
}

/** `1 button`, `2 buttons`: shorthand for a plural family with the number as `count`. */
export function tn(key: TranslationKey, count: number, params?: Params): string {
  return translate(current, key, { ...params, count });
}

/** Date and time formatting uses the interface language. */
export function locale(): string {
  return current;
}

export { en, ru };
