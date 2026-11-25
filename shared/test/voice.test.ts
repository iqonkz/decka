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
  "status",
  "lyrics_window",
  "voice_action",
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

    it(`validates M26 status/lyrics/reply/followup phrases and slots for ${lang}`, () => {
      const table = JSON.parse(fs.readFileSync(path.join(voiceDir, `${lang}.json`), "utf-8"));
      const topics = new Set(
        table.intents
          .filter((i: { tool: string }) => i.tool === "status")
          .map((i: { args?: { topic?: string } }) => i.args?.topic),
      );
      expect(topics).toEqual(
        new Set(["weather", "cpu", "ram", "disk", "network", "phone_battery", "media", "ai_limits", "focus", "next_reminder"]),
      );

      const lyrics = table.intents.filter((i: { tool: string }) => i.tool === "lyrics_window");
      expect(lyrics.some((i: { args?: { show?: boolean } }) => i.args?.show === false)).toBe(true);
      expect(lyrics.some((i: { args?: { show?: boolean } }) => i.args?.show === true)).toBe(true);

      const translate = table.intents.find(
        (i: { tool: string; args?: { op?: string } }) => i.tool === "text_transform" && i.args?.op === "translate",
      );
      expect(translate?.patterns.some((p: string) => p.includes("{text}") && p.includes("{lang}"))).toBe(true);
      expect(
        table.intents.some(
          (i: { tool: string; args?: { op?: string } }) => i.tool === "text_transform" && i.args?.op === "reply",
        ),
      ).toBe(true);

      const actions = new Set(
        table.intents
          .filter((i: { tool: string }) => i.tool === "voice_action")
          .map((i: { args?: { kind?: string } }) => i.args?.kind),
      );
      expect(actions).toEqual(new Set(["copy", "insert", "speak", "again"]));

      // Slot syntax & duplicate slotless pattern collision check
      const allowedSlots = new Set(["app", "domain", "expr", "item", "label", "lang", "list", "n", "name", "query", "text", "time"]);
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
