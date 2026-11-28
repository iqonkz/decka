import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { setLanguage } from "@decka/shared";
import GuidesBlock from "../src/tabs/GuidesBlock";
import ModesBlock from "../src/tabs/ModesBlock";
import VoiceSection, { supports9bModel } from "../src/tabs/VoiceSection";

describe("M27 Settings UI (T27.2b)", () => {
  it("renders saved guides list and empty state", () => {
    setLanguage("en");
    const populated = renderToStaticMarkup(
      <GuidesBlock
        initialGuides={[
          {
            id: "g1",
            goal: ["Save as PDF"],
            process: "WINWORD.EXE",
            steps: [{ role: "Button", name: "Export", automationId: "btnExport" }],
            created: "2026-11-25",
            uses: 4,
          },
        ]}
      />,
    );
    expect(populated).toContain("Save as PDF");
    expect(populated).toContain("WINWORD.EXE");
    expect(populated).toContain("Delete");

    const empty = renderToStaticMarkup(<GuidesBlock initialGuides={[]} />);
    expect(empty).toContain("No saved guides yet");
  });

  it("renders work modes list with apps and focus duration", () => {
    setLanguage("en");
    const html = renderToStaticMarkup(
      <ModesBlock
        initialModes={[{ name: "Dev", apps: [{ exe: "code.exe" }, { exe: "chrome.exe" }], focusMinutes: 45 }]}
      />,
    );
    expect(html).toContain("Dev");
    expect(html).toContain("code.exe, chrome.exe");
    expect(html).toContain("Focus 45 min");
    expect(html).toContain("Add mode");
  });

  it("checks hardware requirement and exposes voice.notices + Qwen3.5 9B option", () => {
    setLanguage("en");
    expect(supports9bModel({ ramGb: 8 })).toBe(false);
    expect(supports9bModel({ ramGb: 16 })).toBe(true);
    expect(supports9bModel({ ramGb: 8, vramGb: 8 })).toBe(true);

    const highSpec = renderToStaticMarkup(
      <VoiceSection
        openAdvanced
        initialModelTier="9b"
        initialStatus={{
          state: "ready",
          hardware: { ramGb: 32, gpu: "RTX 4070", vramGb: 12 },
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
    expect(highSpec).toContain("System notices in Island");
    expect(highSpec).toContain("Extended (Qwen3.5 9B)");
    expect(highSpec).toContain("Qwen3.5 9B");

    const lowSpec = renderToStaticMarkup(
      <VoiceSection
        openAdvanced
        initialStatus={{
          state: "ready",
          hardware: { ramGb: 8 },
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
    expect(lowSpec).not.toContain("Extended (Qwen3.5 9B)");
    expect(lowSpec).toContain("16 GB RAM");
  });
});
