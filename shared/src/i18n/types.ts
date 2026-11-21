import type { en } from "./en";

type Base = keyof typeof en;
/** Languages with more plural categories than English (`ru`, `kk`, `ar`, ...) may add `zero`, `two`, `few` and `many` forms of any family. */
type ExtraPlural = Base extends infer K ? (K extends `${infer B}.other` ? `${B}.zero` | `${B}.two` | `${B}.few` | `${B}.many` : never) : never;

/** Month and weekday names are a fallback for languages without `Intl` data (T19.5); only `en`, `ru` and `kk` carry them. */
type DateKey = Extract<Base, `date.${string}`>;

/** A dictionary: every key of `en.ts` is required (a missing key does not compile); extra plural forms and `date.*` are optional. */
export type Dictionary = Record<Exclude<Base, DateKey>, string> & Partial<Record<DateKey | ExtraPlural, string>>;
