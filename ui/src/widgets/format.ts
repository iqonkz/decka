import { getLanguage, locale, t } from "@decka/shared";

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

/** Short date in the interface language, e.g. "Sat, 14 Nov" or "сб, 14 нояб.". */
export function formatDate(d: Date): string {
  return d.toLocaleDateString(locale(), { weekday: "short", day: "numeric", month: "short" });
}
