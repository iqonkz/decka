import { describe, expect, test } from "vitest";
import { en, ru, resolveLanguage, translate, validateLayout, type ActionsFile, type LayoutFile } from "../src";

/** Strings that may be the same in both languages: product and technology names, units that are written the same. */
const SAME_IN_BOTH = new Set<string>(["USB", "Wi-Fi", "QR", "Decka", "AMOLED", "Studio", "Material", "Glass", "Phosphor", "OK", "Decka {version}"]);

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
  test("auto follows the system language: Russian only for ru*", () => {
    expect(resolveLanguage("auto", "ru-RU")).toBe("ru");
    expect(resolveLanguage("auto", "ru")).toBe("ru");
    expect(resolveLanguage("auto", "kk-KZ")).toBe("en");
    expect(resolveLanguage("auto", "en-US")).toBe("en");
    expect(resolveLanguage(undefined, "ru-RU")).toBe("ru");
    expect(resolveLanguage("auto", undefined)).toBe("en");
  });
  test("an explicit choice wins", () => {
    expect(resolveLanguage("en", "ru-RU")).toBe("en");
    expect(resolveLanguage("ru", "en-US")).toBe("ru");
  });
});

describe("the two tables", () => {
  test("every Russian text is not empty and has the same {parameters} as English", () => {
    const params = (s: string) => (s.match(/\{\w+\}/g) ?? []).sort().join(",");
    for (const [key, text] of Object.entries(ru)) {
      expect(text.trim(), key).not.toBe("");
      const source = (en as Record<string, string>)[key];
      if (source !== undefined) expect(params(text), key).toBe(params(source));
    }
  });

  test("Russian differs from English except for names that are written the same", () => {
    for (const [key, source] of Object.entries(en)) {
      const text = (ru as Record<string, string>)[key];
      if (text === source) expect(SAME_IN_BOTH.has(source) || /^[\d\s{}\w.+:×-]*$/.test(source) && !/[A-Za-z]{3,}/.test(source.replace(/\{\w+\}/g, "")), `${key}: "${source}"`).toBe(true);
    }
  });

  test("every plural family has all four Russian forms", () => {
    const bases = new Set(Object.keys(en).filter((k) => k.endsWith(".other")).map((k) => k.slice(0, -6)));
    expect(bases.size).toBeGreaterThan(5);
    for (const base of bases) for (const form of ["one", "few", "many", "other"]) expect(ru, `${base}.${form}`).toHaveProperty(`${base}.${form}`);
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

  test("auto, en and ru are accepted; anything else is an error at /display/language", () => {
    for (const ok of [undefined, "auto", "en", "ru"]) expect(validateLayout(layout(ok), actions), String(ok)).toEqual([]);
    const issues = validateLayout(layout("de"), actions);
    expect(issues.map((i) => i.path)).toEqual(["/display/language"]);
  });
});
