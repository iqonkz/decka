import { describe, it, expect } from "vitest";
import fs from "node:fs";
import path from "node:path";

const VALID_TOOLS = new Set([
  "press_button",
  "run_action",
  "open_app",
  "switch_app",
  "close_app",
  "window_other_monitor",
  "which_monitor",
  "volume",
  "media",
  "go_page",
  "web_search",
  "open_site",
  "type_text",
  "dictate",
  "timer",
  "system",
  "text_transform",  "screen_ask",  "remind",  "reminders_show",  "list_add",  "list_show",  "list_remove",  "day_brief",  "memory_add",  "memory_remove",  "paste_text",
  "calc",
  "ask",
  "lyrics_window",
  "status",
  "reply_action",
]);

const LANGUAGES = ["en", "zh", "hi", "es", "ar", "fr", "pt", "ru", "de", "ja", "kk"];

describe("Voice phrase tables (T21.3b)", () => {
  const voiceDir = path.resolve(__dirname, "../voice");
  const testsDir = path.resolve(__dirname, "../voice/tests");
  const evalDir = path.resolve(__dirname, "../voice/eval");

  for (const lang of LANGUAGES) {
    it(`validates voice table for ${lang}`, () => {
      const filePath = path.join(voiceDir, `${lang}.json`);
      expect(fs.existsSync(filePath)).toBe(true);

      const content = JSON.parse(fs.readFileSync(filePath, "utf-8"));
      expect(content.lang).toBe(lang);
      expect(Array.isArray(content.yes)).toBe(true);
      expect(content.yes.length).toBeGreaterThan(0);
      expect(Array.isArray(content.no)).toBe(true);
      expect(content.no.length).toBeGreaterThan(0);
      expect(Array.isArray(content.place_prepositions)).toBe(true);
      expect(Array.isArray(content.place_postpositions)).toBe(true);
      expect(Array.isArray(content.place_suffixes)).toBe(true);
      expect(Array.isArray(content.place_ignore)).toBe(true);
      expect(
        content.place_prepositions.length + content.place_postpositions.length + content.place_suffixes.length,
      ).toBeGreaterThan(0);

      expect(Array.isArray(content.intents)).toBe(true);
      expect(content.intents.length).toBeGreaterThanOrEqual(10);
      expect(content.intents.length).toBeLessThanOrEqual(60);

      for (const intent of content.intents) {
        expect(VALID_TOOLS.has(intent.tool)).toBe(true);
        expect(Array.isArray(intent.patterns)).toBe(true);
        expect(intent.patterns.length).toBeGreaterThan(0);
        for (const p of intent.patterns) {
          expect(typeof p).toBe("string");
          expect(p.trim().length).toBeGreaterThan(0);
        }
      }
    });

    it(`validates test file for ${lang}`, () => {
      const filePath = path.join(testsDir, `${lang}.json`);
      expect(fs.existsSync(filePath)).toBe(true);

      const tests = JSON.parse(fs.readFileSync(filePath, "utf-8"));
      expect(Array.isArray(tests)).toBe(true);
      expect(tests.length).toBe(10);

      for (const t of tests) {
        expect(typeof t.say).toBe("string");
        expect(t.say.trim().length).toBeGreaterThan(0);
        expect(VALID_TOOLS.has(t.tool)).toBe(true);
        if (t.args) {
          expect(typeof t.args).toBe("object");
        }
      }
    });

    it(`validates eval set for ${lang} (T21.4b)`, () => {
      const filePath = path.join(evalDir, `${lang}.json`);
      expect(fs.existsSync(filePath)).toBe(true);

      const evalSet = JSON.parse(fs.readFileSync(filePath, "utf-8"));
      expect(Array.isArray(evalSet)).toBe(true);
      expect(evalSet.length).toBeGreaterThanOrEqual(25);

      for (const item of evalSet) {
        expect(typeof item.say).toBe("string");
        expect(item.say.trim().length).toBeGreaterThan(0);
        expect(VALID_TOOLS.has(item.tool)).toBe(true);
        expect(typeof item.args).toBe("object");
      }
    });
  }
});
