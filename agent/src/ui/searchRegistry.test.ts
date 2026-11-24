import { describe, expect, it } from "vitest";
import { searchRegistry, targetForPage, type SearchEntry } from "./searchRegistry";
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
