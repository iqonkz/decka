import { createElement } from "react";
import { renderToString } from "react-dom/server";
import { describe, expect, test } from "vitest";
import { setLanguage, type VoiceStatus } from "@decka/shared";
import FirstRun from "../src/tabs/FirstRun";

const mockVoice: VoiceStatus = {
  state: "off",
  hardware: { gpu: "RTX 3060", vramGb: 12, ramGb: 16 },
  model: "standard",
  downloadGb: 3.2,
  hotkey: ["ctrl", "alt", "space"],
  keepLoaded: false,
  unloadAfterMin: 10,
  speak: false,
  phoneButton: true,
  mics: [],
};

describe("T31.4b 3-step PC first-run wizard (Phone -> Voice -> Done)", () => {
  test("renders all 3 steps with step pills, Skip button on every step, Voice tier/download, and 3 example phrases", () => {
    setLanguage("ru");

    const step0 = renderToString(createElement(FirstRun, { onClose: () => {}, startStep: 0, initialVoiceStatus: mockVoice }));
    expect(step0).toContain("wizard-topbar");
    expect(step0).toContain("wizard-step-pill active");
    expect(step0).toContain("Добро пожаловать в Decka");
    expect(step0).toContain("Пропустить");

    const step1 = renderToString(
      createElement(FirstRun, { onClose: () => {}, startStep: 1, initialVoiceStatus: { ...mockVoice, suggestLight: true } }),
    );
    expect(step1).toContain("settings-page-header");
    expect(step1).toContain("settings-group-card");
    expect(step1).toContain("Голос");
    expect(step1).toContain("Рекомендуется");
    expect(step1).toContain("Пропустить");

    const step2 = renderToString(createElement(FirstRun, { onClose: () => {}, startStep: 2, initialVoiceStatus: mockVoice }));
    expect(step2).toContain("settings-page-header");
    expect(step2).toContain("Готово");
    expect(step2).toContain("Сделай громче");
    expect(step2).toContain("Какая погода завтра");
    expect(step2).toContain("Запиши заметку о созвоне");
    expect(step2).toContain("Пропустить");
    expect(step2).toContain("Закончить");
  });
});
