import type { en } from "./en";

type Base = keyof typeof en;
/** Languages with more plural categories than English (`ru`, `kk`, `ar`, ...) may add `zero`, `two`, `few` and `many` forms of any family. */
type ExtraPlural = Base extends infer K ? (K extends `${infer B}.other` ? `${B}.zero` | `${B}.two` | `${B}.few` | `${B}.many` : never) : never;

/** A dictionary: every key of `en.ts` is required (a missing key does not compile); extra plural forms are optional. */
export type Dictionary = Record<Base, string> & Partial<Record<ExtraPlural, string>>;
