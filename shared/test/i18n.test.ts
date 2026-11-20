import { describe, expect, test } from "vitest";
import { CATALOG, LANGUAGES, LANGUAGE_NAMES, SETS, TABLES, en, isRtl, languageOrder, pickText, resolveLanguage, translate, validateLayout, type ActionsFile, type Language, type LayoutFile } from "../src";

/**
 * The languages whose dictionaries are translated (the `TODO translate` header is gone). The strict checks run for these only;
 * each translation task of M19 adds its languages here.
 */
const TRANSLATED: Language[] = ["ru", "es", "kk"];

/** Strings that may be the same in every language: product and technology names, units that are written the same. */
const SAME_IN_ALL = new Set<string>(["USB", "Wi-Fi", "QR", "Decka", "AMOLED", "Studio", "Material", "Glass", "Phosphor", "OK", "Decka {version}", "OBS", "OBS Studio", "Claude", "Codex", "Claude Code", "Twitch", "YouTube", "REC", "LIVE", "Weather data by Open-Meteo.com (CC BY 4.0)"]);
/** Short strings of letters that are the same in many languages (a unit, a code, a brand): not counted as untranslated. */
const looksUniversal = (s: string) => SAME_IN_ALL.has(s) || (/^[\d\s{}\w.+:×/%-]*$/.test(s) && !/[A-Za-z]{4,}/.test(s.replace(/\{\w+\}/g, "")));

describe("translate", () => {
  test("parameters are substituted; unknown ones stay as written", () => {
    expect(translate("en", "pairing.paired", { name: "Galaxy" })).toBe("Paired: Galaxy");
    expect(translate("ru", "pairing.paired", { name: "Galaxy" })).toBe("Подключено: Galaxy");
    expect(translate("en", "pairing.paired", {})).toBe("Paired: {name}");
  });

  test("Russian plural forms: кнопка / кнопки / кнопок", () => {
    const form = (n: number) => translate("ru", "grid.keys", { count: n });
    expect(form(1)).toBe("1 кнопка");
    expect(form(2)).toBe("2 кнопки");
    expect(form(5)).toBe("5 кнопок");
    expect(form(11)).toBe("11 кнопок");
    expect(form(21)).toBe("21 кнопка");
    expect(form(22)).toBe("22 кнопки");
    expect(form(25)).toBe("25 кнопок");
  });

  test("English plural forms", () => {
    expect(translate("en", "grid.keys", { count: 1 })).toBe("1 key");
    expect(translate("en", "grid.keys", { count: 0 })).toBe("0 keys");
    expect(translate("en", "grid.keys", { count: 2 })).toBe("2 keys");
  });

  test("a language with fewer plural forms (Chinese has only `other`) still finds a text, and a missing form falls back to other, then one", () => {
    const sample = Object.keys(en).find((k) => k.endsWith(".other"))!.slice(0, -6) as Parameters<typeof translate>[1];
    for (const lang of LANGUAGES) {
      for (const n of [0, 1, 2, 3, 5, 11, 21, 100]) expect(translate(lang, sample, { count: n }), `${lang} ${n}`).not.toBe(sample);
    }
  });

  test("the glossary of DESIGN.md 6 is used", () => {
    expect(translate("ru", "tab.deck")).toBe("Панель");
    expect(translate("ru", "tab.look")).toBe("Вид");
    expect(translate("ru", "tab.phone")).toBe("Телефон");
    expect(translate("ru", "tab.more")).toBe("Ещё");
    expect(translate("ru", "menu.unpair")).toBe("Отключить от ПК");
    expect(translate("ru", "menu.connected", { pc: "DESKTOP" })).toBe("Подключено к DESKTOP");
    expect(translate("ru", "widget.nothing_playing")).toBe("Ничего не играет");
    expect(translate("ru", "widget.no_notifications")).toBe("Нет уведомлений");
    expect(translate("ru", "menu_item.big_icon")).toBe("Большая иконка");
    expect(translate("ru", "pairing.forget")).toBe("Забыть");
  });
});

describe("resolveLanguage", () => {
  test("auto takes the first part of the system language", () => {
    expect(resolveLanguage("auto", "ru-RU")).toBe("ru");
    expect(resolveLanguage("auto", "ru")).toBe("ru");
    expect(resolveLanguage("auto", "zh-TW")).toBe("zh");
    expect(resolveLanguage("auto", "pt-PT")).toBe("pt");
    expect(resolveLanguage("auto", "kk-KZ")).toBe("kk");
    expect(resolveLanguage("auto", "de_AT")).toBe("de");
    expect(resolveLanguage("auto", "AR-sa")).toBe("ar");
    expect(resolveLanguage("auto", "sw")).toBe("en");
    expect(resolveLanguage("auto", "en-US")).toBe("en");
    expect(resolveLanguage(undefined, "ja-JP")).toBe("ja");
    expect(resolveLanguage("auto", undefined)).toBe("en");
    expect(resolveLanguage("auto", "")).toBe("en");
  });
  test("an explicit choice wins; an unknown code is `auto`", () => {
    expect(resolveLanguage("en", "ru-RU")).toBe("en");
    expect(resolveLanguage("ru", "en-US")).toBe("ru");
    expect(resolveLanguage("hi", "en-US")).toBe("hi");
    expect(resolveLanguage("xx", "fr-FR")).toBe("fr");
  });
});

describe("the list of languages", () => {
  test("eleven languages, each named in itself", () => {
    expect([...LANGUAGES].sort()).toEqual(["ar", "de", "en", "es", "fr", "hi", "ja", "kk", "pt", "ru", "zh"]);
    expect(LANGUAGE_NAMES.kk).toBe("Қазақша");
    expect(LANGUAGE_NAMES.en).toBe("English");
  });
  test("the language of the system and English come first, then the rest by name", () => {
    expect(languageOrder("ru").slice(0, 2)).toEqual(["ru", "en"]);
    expect(languageOrder("en")[0]).toBe("en");
    expect(new Set(languageOrder("kk")).size).toBe(11);
  });
  test("only Arabic is written from right to left", () => {
    expect(LANGUAGES.filter(isRtl)).toEqual(["ar"]);
  });
});

describe("the dictionaries", () => {
  const params = (s: string) => (s.match(/\{\w+\}/g) ?? []).sort().join(",");

  test.each(LANGUAGES.filter((l) => l !== "en"))("%s has every key of en, none empty, and the same {parameters}", (lang) => {
    const table = TABLES[lang];
    for (const [key, source] of Object.entries(en)) {
      const text = table[key];
      expect(text, `${lang}: ${key}`).toBeDefined();
      expect(text!.trim(), `${lang}: ${key}`).not.toBe("");
      expect(params(text!), `${lang}: ${key}`).toBe(params(source));
    }
    // Extra keys are only extra plural forms.
    for (const key of Object.keys(table)) if (!(key in en)) expect(key, `${lang}: ${key}`).toMatch(/\.(zero|two|few|many)$/);
  });

  test.each(TRANSLATED.filter((l) => l !== "en"))("%s is translated: at most 20% of the texts equal English (names and codes excepted)", (lang) => {
    const table = TABLES[lang];
    const same = Object.entries(en).filter(([key, source]) => table[key] === source && !looksUniversal(source));
    expect(same.length / Object.keys(en).length, same.slice(0, 8).map(([k]) => k).join(", ")).toBeLessThanOrEqual(0.2);
  });

  test("Russian differs from English except for names that are written the same", () => {
    for (const [key, source] of Object.entries(en)) {
      if (TABLES.ru[key] === source) expect(looksUniversal(source), `${key}: "${source}"`).toBe(true);
    }
  });

  test("every plural family has all four Russian forms", () => {
    const bases = new Set(Object.keys(en).filter((k) => k.endsWith(".other")).map((k) => k.slice(0, -6)));
    expect(bases.size).toBeGreaterThan(5);
    for (const base of bases) for (const form of ["one", "few", "many", "other"]) expect(TABLES.ru, `${base}.${form}`).toHaveProperty(`${base}.${form}`);
  });

  test("a translated language has the plural forms its rules use", () => {
    const bases = [...new Set(Object.keys(en).filter((k) => k.endsWith(".other")).map((k) => k.slice(0, -6)))];
    for (const lang of TRANSLATED) {
      // The categories the language really uses for whole numbers (Spanish and French have `many` only for millions).
      const rules = new Intl.PluralRules(lang);
      const categories = new Set(Array.from({ length: 201 }, (_, n) => rules.select(n)));
      for (const base of bases) for (const category of categories) expect(TABLES[lang], `${lang}: ${base}.${category}`).toHaveProperty(`${base}.${category}`);
    }
  });
});

describe("the data of the catalog in the languages", () => {
  test("pickText falls back to English", () => {
    expect(pickText({ en: "Lock", ru: "Блок" }, "ru")).toBe("Блок");
    expect(pickText({ en: "Lock", ru: "Блок" }, "ja")).toBe("Lock");
    expect(pickText(undefined, "ja")).toBe("");
  });

  test.each(TRANSLATED)("%s: every text of the catalog and of the sets is there", (lang) => {
    for (const c of CATALOG.categories) expect(c.name[lang], `category ${c.id}`).toBeTruthy();
    for (const item of CATALOG.items) {
      expect(item.name[lang], `${item.id} name`).toBeTruthy();
      expect(item.label[lang], `${item.id} label`).toBeTruthy();
      for (const [key, state] of Object.entries(item.bind?.states ?? {})) if (state.label) expect(state.label[lang], `${item.id} state ${key}`).toBeTruthy();
    }
    for (const set of SETS) {
      expect(set.name[lang], `set ${set.id}`).toBeTruthy();
      expect(set.description[lang], `set ${set.id} description`).toBeTruthy();
    }
  });
});

describe("display.language in the contract", () => {
  const layout = (language: unknown): LayoutFile =>
    ({
      version: 1,
      grid: { cols: 7, rows: 4 },
      display: { dimAfterSec: 60, dimLevel: 0.3, sleepAfterSec: 600, pixelShift: true, night: { enabled: false, from: "23:00", to: "07:00" }, language },
      pages: [{ id: "main", name: "Main", items: [] }],
    }) as unknown as LayoutFile;
  const actions: ActionsFile = { version: 1, actions: {} };

  test("auto and the eleven languages are accepted; anything else is an error at /display/language", () => {
    for (const ok of [undefined, "auto", ...LANGUAGES]) expect(validateLayout(layout(ok), actions), String(ok)).toEqual([]);
    const issues = validateLayout(layout("xx"), actions);
    expect(issues.map((i) => i.path)).toEqual(["/display/language"]);
  });
});
