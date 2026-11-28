import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { setLanguage } from "@decka/shared";
import VoiceCheckPanel from "../src/tabs/VoiceCheckPanel";
import VoiceSection from "../src/tabs/VoiceSection";
import { voiceErrorCopy, type VoiceErrorReason } from "../src/tabs/voiceM28Types";

describe("M28 Settings UI (T28.2b)", () => {
  it("provides localized short/settings error strings and actions for all 7 voice error reasons", () => {
    const reasons: VoiceErrorReason[] = ["vram", "ram", "model_file", "port", "crash", "mic", "stt"];
    setLanguage("ru");
    expect(voiceErrorCopy("vram").short).toBe("Не хватило памяти для модели");
    expect(voiceErrorCopy("model_file").buttonLabel).toBe("Скачать заново");
    expect(voiceErrorCopy("crash", 137).reason).toContain("137");
    for (const r of reasons) {
      const c = voiceErrorCopy(r, 1);
      expect(c.short.trim().length).toBeGreaterThan(0);
      expect(c.reason.trim().length).toBeGreaterThan(0);
      expect(c.hint.trim().length).toBeGreaterThan(0);
      expect(c.buttonLabel.trim().length).toBeGreaterThan(0);
    }

    setLanguage("en");
    expect(voiceErrorCopy("vram").short).toBe("Not enough GPU memory for the model");
    expect(voiceErrorCopy("model_file").buttonLabel).toBe("Redownload");
  });

  it("renders error reason, hint, and action button in VoiceSection error state", () => {
    setLanguage("ru");
    const html = renderToStaticMarkup(
      <VoiceSection
        initialStatus={{
          state: "error",
          reason: "model_file",
          hardware: { ramGb: 8, vramGb: 2 },
          model: "standard",
          downloadGb: 3.2,
          hotkey: ["ctrl", "alt", "space"],
          keepLoaded: false,

          unloadAfterMin: 10,
          speak: false,
          speechPercent: 105, phoneButton: true,
          mics: [],
        }}
      />,
    );
    expect(html).toContain("Файл модели не читается или повреждён.");
    expect(html).toContain("Скачать заново");
    expect(html).toContain("Проверить голос");
  });

  it("renders Screen assist toggle row and 4-step Check voice panel in VoiceSection", () => {
    setLanguage("ru");
    const html = renderToStaticMarkup(
      <VoiceSection
        initialContext
        initialCheckSteps={[
          { id: "mic", state: "ok", durationMs: 3000, detail: "Уровень сигнала в норме" },
          { id: "stt", state: "ok", durationMs: 420, detail: "«который час»" },
          { id: "model", state: "error", durationMs: 180, reason: "vram" },
          { id: "action", state: "pending" },
        ]}
        initialStatus={{
          state: "ready",
          hardware: { ramGb: 8, gpu: "GTX 1050", vramGb: 2 },
          model: "light",
          downloadGb: 1.8,
          hotkey: ["ctrl", "alt", "space"],
          keepLoaded: false,

          unloadAfterMin: 10,
          speak: false,
          speechPercent: 105, phoneButton: true,
          mics: ["USB Mic"],
        }}
      />,
    );
    expect(html).toContain("1. Микрофон");
    expect(html).toContain("2. Распознавание");
    expect(html).toContain("3. Модель");
    expect(html).toContain("4. Действие");
    expect(html).toContain("ОК · 3.0 s");
    expect(html).toContain("Ошибка · 180 ms");
  });

  it("renders standalone VoiceCheckPanel in English", () => {
    setLanguage("en");
    const html = renderToStaticMarkup(
      <VoiceCheckPanel
        initialSteps={[
          { id: "mic", state: "ok", durationMs: 3000 },
          { id: "stt", state: "ok", durationMs: 350, detail: "“what time is it”" },
          { id: "model", state: "ok", durationMs: 210, detail: "status (time)" },
          { id: "action", state: "ok", durationMs: 12, detail: "Dry run: speak current time" },
        ]}
      />,
    );
    expect(html).toContain("Check voice");
    expect(html).toContain("1. Microphone");
    expect(html).toContain("2. Recognition");
    expect(html).toContain("3. Model");
    expect(html).toContain("4. Action");
    expect(html).toContain("Dry run: speak current time");
  });
});
