import { describe, expect, it } from "vitest";
import { SEARCH_REGISTRY, anchorId, revealAnchor, searchRegistry, targetForPage, type SearchEntry } from "./searchRegistry";
import { TABLES } from "../../../shared/src/i18n";
import { en } from "../../../shared/src/i18n/en";
import { ru } from "../../../shared/src/i18n/ru";

describe("searchRegistry (T23.1b)", () => {
  const enTranslate = (key: string) => (en as Record<string, string>)[key] ?? key;
  const ruTranslate = (key: string) => (ru as Record<string, string>)[key] ?? key;

  it("handles case-insensitive search", () => {
    const resLower = searchRegistry("voice", undefined, enTranslate);
    const resUpper = searchRegistry("VOICE", undefined, enTranslate);
    const resMixed = searchRegistry("VoIcE", undefined, enTranslate);

    expect(resLower.length).toBeGreaterThan(0);
    expect(resLower.map((r) => r.id)).toEqual(resUpper.map((r) => r.id));
    expect(resLower.map((r) => r.id)).toEqual(resMixed.map((r) => r.id));
  });

  it("handles partial match in English", () => {
    const res = searchRegistry("remind", undefined, enTranslate);
    expect(res.some((r) => r.id === "reminders")).toBe(true);
  });

  it("handles partial match in Russian", () => {
    const res = searchRegistry("голос", undefined, ruTranslate);
    expect(res.some((r) => r.id === "voice")).toBe(true);

    const resRem = searchRegistry("напом", undefined, ruTranslate);
    expect(resRem.some((r) => r.id === "reminders")).toBe(true);
  });

  it("finds the settings by the words of the window language (T23.B)", () => {
    const pages = (q: string) => searchRegistry(q, undefined, ruTranslate).map((r) => r.page);
    expect(pages("микрофон")).toContain("voice");
    expect(pages("тема")).toContain("look");
    expect(pages("порт")).toContain("settings");
    expect(searchRegistry("почта", undefined, ruTranslate).some((r) => r.id === "integrations-mail" && r.page === "integrations")).toBe(true);
    // The row of the setting comes before the page that holds it.
    expect(searchRegistry("микрофон", undefined, ruTranslate)[0]?.id).toBe("voice-mic");
    expect(searchRegistry("microphone", undefined, enTranslate)[0]?.id).toBe("voice-mic");
    // A name of a setting finds the page even when it has no row of its own.
    expect(pages("язык")).toContain("settings");
    expect(pages("напоминания")).toContain("reminders");
  });

  it("has a name for every row and every searched setting in all 11 languages", () => {
    const tables = TABLES as Record<string, Record<string, string>>;
    expect(Object.keys(tables)).toHaveLength(11);
    for (const [lang, table] of Object.entries(tables)) {
      for (const entry of SEARCH_REGISTRY) {
        for (const key of [entry.titleKey, ...(entry.keyKeys ?? [])]) {
          expect(table[key], `${lang}: ${key}`).toBeTruthy();
        }
      }
      // Every page is found by its own name in its own language.
      const translate = (key: string) => table[key] ?? key;
      for (const entry of SEARCH_REGISTRY.filter((e) => e.id === e.page || e.id === "keys")) {
        const name = translate(entry.titleKey);
        expect(searchRegistry(name, undefined, translate).some((r) => r.id === entry.id), `${lang}: ${name}`).toBe(true);
      }
    }
  });

  it("scrolls to and highlights the microphone row for «микрофон» (T23.R2)", () => {
    const top = searchRegistry("микрофон", undefined, ruTranslate)[0]!;
    expect([top.id, top.page]).toEqual(["voice-mic", "voice"]);
    expect(anchorId(top)).toBe("section-voice-mic");
    const classes = new Set<string>();
    const scrolled: unknown[] = [];
    const row = {
      scrollIntoView: (o: unknown) => scrolled.push(o),
      classList: { add: (c: string) => classes.add(c), remove: (c: string) => classes.delete(c) },
    };
    const doc = { getElementById: (id: string) => (id === "section-voice-mic" ? row : null) } as unknown as Document;
    expect(revealAnchor(top, doc, 5)).toBe(true);
    expect(scrolled).toEqual([{ behavior: "smooth", block: "center" }]);
    expect(classes.has("search-highlight")).toBe(true);
    return new Promise<void>((done) => setTimeout(() => { expect(classes.size).toBe(0); done(); }, 30));
  });

  it("every anchor of the registry is an id on its page", () => {
    const files = import.meta.glob("../tabs/*.tsx", { query: "?raw", import: "default", eager: true }) as Record<string, string>;
    const source = Object.values(files).join("\n");
    for (const entry of SEARCH_REGISTRY.filter((e) => e.anchor)) {
      const id = anchorId(entry)!;
      const dynamic = id.startsWith("section-integrations-") ? "section-integrations-${item.id}" : null;
      expect(source.includes(`id="${id}"`) || (dynamic !== null && source.includes(dynamic)), id).toBe(true);
    }
    const empty = { getElementById: () => null } as unknown as Document;
    expect(revealAnchor({ id: "x", page: "about", titleKey: "sidebar.item.about" }, empty)).toBe(false);
  });

  it("limits results to at most 8 items", () => {
    // A mock registry with 15 matching items
    const manyEntries: SearchEntry[] = Array.from({ length: 15 }, (_, i) => ({
      id: `item-${i}`,
      page: "settings",
      titleKey: "sidebar.item.settings",
      keywords: ["common-keyword"],
    }));

    const res = searchRegistry("common", manyEntries, () => "test");
    expect(res.length).toBe(8);
  });

  it("returns empty array for non-matching or empty query", () => {
    expect(searchRegistry("")).toEqual([]);
    expect(searchRegistry("   ")).toEqual([]);
    expect(searchRegistry("xyznonexistentterm12345")).toEqual([]);
  });

  it("correctly maps each page target to tab and optional section", () => {
    expect(targetForPage("deck")).toEqual({ tab: "deck" });
    expect(targetForPage("look")).toEqual({ tab: "look" });
    expect(targetForPage("phone")).toEqual({ tab: "phone" });
    expect(targetForPage("voice")).toEqual({ tab: "more", section: "voice" });
    expect(targetForPage("reminders")).toEqual({ tab: "more", section: "reminders" });
    expect(targetForPage("integrations")).toEqual({ tab: "more", section: "integrations" });
    expect(targetForPage("actions")).toEqual({ tab: "more", section: "actions" });
    expect(targetForPage("settings")).toEqual({ tab: "more", section: "settings" });
    expect(targetForPage("about")).toEqual({ tab: "more", section: "about" });
  });
});
