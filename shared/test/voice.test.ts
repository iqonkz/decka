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
  "find_on_screen",
  "ui_act",
  "guide",
  "guide_record",
  "read_aloud",
  "mode_run",
  "mode_save",
  "guide_control",
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

      expect(Array.isArray(content.intents)).toBe(true);
      expect(content.intents.length).toBeGreaterThanOrEqual(10);
      expect(content.intents.length).toBeLessThanOrEqual(120);

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

    it(`validates M27 screen/ui_act/guide/controls/danger_words phrases and slots for ${lang}`, () => {
      const table = JSON.parse(fs.readFileSync(path.join(voiceDir, `${lang}.json`), "utf-8"));
      expect(table.intents.some((i: { tool: string }) => i.tool === "find_on_screen")).toBe(true);
      expect(table.intents.some((i: { tool: string; args?: { next?: boolean } }) => i.tool === "find_on_screen" && i.args?.next === true)).toBe(true);

      const uiActions = new Set(
        table.intents
          .filter((i: { tool: string }) => i.tool === "ui_act")
          .map((i: { args?: { action?: string } }) => i.args?.action),
      );
      expect(uiActions).toEqual(new Set(["click", "scroll_down", "scroll_up", "close_tab"]));

      expect(table.intents.some((i: { tool: string }) => i.tool === "guide")).toBe(true);
      expect(table.intents.some((i: { tool: string }) => i.tool === "guide_record")).toBe(true);
      expect(table.intents.some((i: { tool: string }) => i.tool === "read_aloud")).toBe(true);
      expect(table.intents.some((i: { tool: string }) => i.tool === "mode_run")).toBe(true);
      expect(table.intents.some((i: { tool: string }) => i.tool === "mode_save")).toBe(true);

      expect(Array.isArray(table.guide_controls?.next) && table.guide_controls.next.length > 0).toBe(true);
      expect(Array.isArray(table.guide_controls?.back) && table.guide_controls.back.length > 0).toBe(true);
      expect(Array.isArray(table.guide_controls?.stop) && table.guide_controls.stop.length > 0).toBe(true);
      expect(Array.isArray(table.guide_controls?.done) && table.guide_controls.done.length > 0).toBe(true);

      expect(Array.isArray(table.danger_words)).toBe(true);
      expect(table.danger_words.length).toBeGreaterThanOrEqual(7);

      const allowedSlots = new Set([
        "app", "domain", "expr", "goal", "item", "label", "lang", "list", "n", "name", "query", "target", "text", "time",
      ]);
      const seenLiterals = new Map<string, string>();
      for (const intent of table.intents) {
        const key = `${intent.tool}:${JSON.stringify(intent.args ?? {})}`;
        for (const p of intent.patterns) {
          const slots = [...p.matchAll(/\{([^}]*)\}/g)].map((m: RegExpMatchArray) => m[1]!);
          expect(p.split("{").length).toBe(p.split("}").length);
          for (const s of slots) expect(allowedSlots.has(s)).toBe(true);
          if (slots.length === 0) {
            const norm = p.toLowerCase().trim().replace(/\s+/g, " ");
            const prev = seenLiterals.get(norm);
            if (prev) expect(prev).toBe(key);
            seenLiterals.set(norm, key);
          }
        }
      }
    });
  }
});
