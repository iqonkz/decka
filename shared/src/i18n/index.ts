import { en } from "./en";
import { ru } from "./ru";
import { zh } from "./zh";
import { hi } from "./hi";
import { es } from "./es";
import { ar } from "./ar";
import { fr } from "./fr";
import { pt } from "./pt";
import { de } from "./de";
import { ja } from "./ja";
import { kk } from "./kk";
import type { LanguageSetting } from "../layout";
import { isLanguage, type Language } from "../languages";

export { LANGUAGES, LANGUAGE_NAMES, isLanguage, isRtl, languageOrder, type Language } from "../languages";

export type MessageKey = keyof typeof en;
type PluralCategory = "zero" | "one" | "two" | "few" | "many" | "other";
/** `keys_count` for the plural family `keys_count.one`, `keys_count.other`, ... */
export type PluralBase<K extends string = MessageKey> = K extends `${infer B}.${PluralCategory}` ? B : never;
/** A key `t()` accepts: a plain message or the base name of a plural family (then `params.count` picks the form). */
export type TranslationKey = Exclude<MessageKey, `${string}.${PluralCategory}`> | PluralBase;
export type Params = Record<string, string | number>;

export const TABLES: Record<Language, Record<string, string>> = { en, ru, zh, hi, es, ar, fr, pt, de, ja, kk };

/**
 * The language of a setting. `auto` (or anything unknown) follows the system language: its first part (`zh-TW` -> `zh`,
 * `pt-PT` -> `pt`); a language that is not in the list gives English (DESIGN.md 6).
 */
export function resolveLanguage(setting: LanguageSetting | string | undefined, systemLanguage: string | undefined): Language {
  if (isLanguage(setting)) return setting;
  const first = (systemLanguage ?? "").toLowerCase().split(/[-_]/)[0];
  return isLanguage(first) ? first : "en";
}

const rules = new Map<Language, Intl.PluralRules>();
function pluralRules(language: Language): Intl.PluralRules {
  let r = rules.get(language);
  if (!r) {
    r = new Intl.PluralRules(language);
    rules.set(language, r);
  }
  return r;
}

function fill(template: string, params: Params | undefined): string {
  if (!params) return template;
  return template.replace(/\{(\w+)\}/g, (whole, name: string) => (name in params ? String(params[name]) : whole));
}

/**
 * The text of `key` in `language`; a plural family is chosen by `params.count` with `Intl.PluralRules` of the language: the form
 * of the category, then `other`, then `one`. A missing text falls back to English, then to the key.
 */
export function translate(language: Language, key: TranslationKey, params?: Params): string {
  const table = TABLES[language];
  let found: string | undefined;
  if (typeof params?.count === "number") {
    const category = pluralRules(language).select(params.count);
    found = table[`${key}.${category}`] ?? table[`${key}.other`] ?? table[`${key}.one`];
  }
  found ??= table[key] ?? TABLES.en[key] ?? TABLES.en[`${key}.other`] ?? key;
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

/**
 * A text of the voice section (PROTOCOL.md 3): the dictionary text of `key` with `args` when the key is known, else the
 * free text `fallback` (a model answer, a program name, a key of a newer agent).
 */
export function tVoice(key: string | undefined, args: Params | undefined, fallback: string | undefined): string | undefined {
  if (key && key in en) return translate(current, key as TranslationKey, args);
  return fallback;
}

/** `1 button`, `2 buttons`: shorthand for a plural family with the number as `count`. */
export function tn(key: TranslationKey, count: number, params?: Params): string {
  return translate(current, key, { ...params, count });
}

/** Date and time formatting uses the interface language. */
export function locale(): string {
  return current;
}

export { en, ru, zh, hi, es, ar, fr, pt, de, ja, kk };
