import { describe, it, expect } from "vitest";
import { getExamplesForCategory } from "./VoiceHelp";
import en from "../../../shared/voice/en.json";
import ru from "../../../shared/voice/ru.json";
import kk from "../../../shared/voice/kk.json";

describe("VoiceHelp 'What you can say' (T21.6b)", () => {
  it("extracts examples for Apps category in English", () => {
    const examples = getExamplesForCategory(en, ["open_app", "close_app", "switch_app"], 3);
    expect(examples.length).toBeGreaterThanOrEqual(2);
    expect(examples.some((ex) => ex.toLowerCase().includes("calculator"))).toBe(true);
  });

  it("extracts examples for Media category in Russian", () => {
    const examples = getExamplesForCategory(ru, ["volume", "media"], 3);
    expect(examples.length).toBeGreaterThanOrEqual(2);
    expect(examples.some((ex) => ex.includes("громче") || ex.includes("пауза"))).toBe(true);
  });

  it("extracts examples for System category in Kazakh", () => {
    const examples = getExamplesForCategory(kk, ["timer", "web_search", "system"], 3);
    expect(examples.length).toBeGreaterThanOrEqual(2);
    expect(examples.some((ex) => ex.includes("таймер") || ex.includes("құлыптау"))).toBe(true);
  });
});
