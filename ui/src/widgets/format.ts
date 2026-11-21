import { getLanguage, locale, t, translate, type Language, type MessageKey, type TranslationKey } from "@decka/shared";

/** "<n> Kbps" below 1000, otherwise "<n.n> Mbps" (Кбит/с, Мбит/с in Russian, with a decimal comma). */
export function formatRate(kbps: number): string {
  if (kbps < 1000) return t("unit.kbps", { n: Math.round(kbps) });
  // The decimal separator of the language, with Latin digits everywhere (Arabic and Hindi digits would not match the rest of the widget).
  const mbps = new Intl.NumberFormat(`${getLanguage()}-u-nu-latn`, { minimumFractionDigits: 1, maximumFractionDigits: 1 }).format(kbps / 1000);
  return t("unit.mbps", { n: mbps });
}

/** Integer percentage; 0 when `total` is 0. */
export function percent(used: number, total: number): number {
  return total === 0 ? 0 : Math.round((used / total) * 100);
}

const two = (n: number): string => String(n).padStart(2, "0");

/** 24-hour clock: HH:MM, or HH:MM:SS with `seconds`. */
export function formatClock(d: Date, seconds: boolean): string {
  const base = `${two(d.getHours())}:${two(d.getMinutes())}`;
  return seconds ? `${base}:${two(d.getSeconds())}` : base;
}

/** The part of `Intl` the date formatting uses (replaceable in tests). */
export interface IntlLike {
  DateTimeFormat: typeof Intl.DateTimeFormat;
}

/**
 * Whether `Intl` knows the names of months and days in `lang`. Chrome and WebView ship no data for some languages (`kk`): they
 * are listed as supported but give `M11 21, Sat`, so the result is checked for such a pattern.
 */
export function hasDateData(lang: string, intl: IntlLike = Intl): boolean {
  try {
    if (intl.DateTimeFormat.supportedLocalesOf(lang).length === 0) return false;
    const sample = new intl.DateTimeFormat(lang, { weekday: "short", day: "numeric", month: "short" }).format(new Date(2026, 10, 21));
    return !/M\d/.test(sample);
  } catch {
    return false;
  }
}

/** Short date in the interface language, e.g. "Sat, 14 Nov", "сб, 14 нояб." or, without `Intl` data, "21 қараша, сб". */
export function formatDate(d: Date, lang: Language = getLanguage(), intl: IntlLike = Intl): string {
  if (hasDateData(lang, intl)) return d.toLocaleDateString(locale(), { weekday: "short", day: "numeric", month: "short" });
  return translate(lang, "date.format", {
    weekday: translate(lang, `date.weekday_short.${d.getDay()}` as MessageKey as TranslationKey),
    day: d.getDate(),
    month: translate(lang, `date.month.${d.getMonth() + 1}` as MessageKey as TranslationKey),
  });
}

/** Short name of the weekday of `d` in the interface language (from the dictionary when `Intl` has no data). */
export function formatWeekday(d: Date, lang: Language = getLanguage(), intl: IntlLike = Intl): string {
  if (hasDateData(lang, intl)) return new intl.DateTimeFormat(lang, { weekday: "short" }).format(d);
  return translate(lang, `date.weekday_short.${d.getDay()}` as MessageKey as TranslationKey);
}
