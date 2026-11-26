import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { CATALOG, setLanguage } from "@decka/shared";
import VoiceSection from "../src/tabs/VoiceSection";
import VoiceCommands from "../src/tabs/VoiceCommands";
import { VOICE_COMMAND_GROUPS, WHITELISTED_VOICE_TOOLS } from "../src/tabs/voiceCommandsData";

describe("M29.10b Voice & Island settings + Catalog", () => {
  it("renders duckMusic, speak, and Island resting mode / bar height / bar position controls", () => {
    setLanguage("ru");
    const html = renderToStaticMarkup(
      <VoiceSection
        initialStatus={{
          state: "ready",
          hardware: { ramGb: 16 },
          model: "standard",
          downloadGb: 3.2,
          hotkey: ["ctrl", "alt", "space"],
          keepLoaded: false,
          speak: true,
          phoneButton: true,
          mics: [],
        }}
        initialDuckMusic={true}
        initialIslandRest="bar"
        initialIslandBarHeight={10}
        initialIslandBarPosition="center"
      />,
    );

    expect(html).toContain("Приглушать музыку, пока слушаю");
    expect(html).toContain("Отвечать голосом");
    expect(html).toContain("Покой острова");
    expect(html).toContain("Высота полосы (px)");
    expect(html).toContain("10 px");
    expect(html).toContain("Позиция полосы");
  });

  it("includes window screenshot, region screenshot, and webhook in the catalog", () => {
    const winShot = CATALOG.items.find((i) => i.id === "win.screenshot.window");
    const regShot = CATALOG.items.find((i) => i.id === "win.screenshot.region");
    const webhook = CATALOG.items.find((i) => i.id === "web.webhook");

    expect(winShot).toBeTruthy();
    expect(winShot?.action).toEqual({ type: "hotkey", keys: ["alt", "printscreen"] });
    expect(regShot).toBeTruthy();
    expect(regShot?.action).toEqual({ type: "hotkey", keys: ["win", "shift", "s"] });
    expect(webhook).toBeTruthy();
    expect(webhook?.ask).toBe("url");
  });
});

describe("M29.12b Voice command reference", () => {
  it("covers every whitelisted voice tool in a group with at least one example", () => {
    expect(VOICE_COMMAND_GROUPS).toHaveLength(11);
    const covered = new Set(
      VOICE_COMMAND_GROUPS.flatMap((g) => g.examples.map((ex) => ex.tool)),
    );
    for (const tool of WHITELISTED_VOICE_TOOLS) {
      expect(covered.has(tool), `Missing command example for tool: ${tool}`).toBe(true);
    }
    for (const g of VOICE_COMMAND_GROUPS) {
      expect(g.examples.length, `Group ${g.id} should have 4-9 examples`).toBeGreaterThanOrEqual(4);
    }
  });

  it("renders 11 collapsed groups by default and expands matching groups when searching", () => {
    setLanguage("ru");
    const collapsedHtml = renderToStaticMarkup(<VoiceCommands />);
    expect(collapsedHtml).toContain("Что можно сказать");
    expect(collapsedHtml).toContain("Музыка и звук");
    expect(collapsedHtml).toContain('aria-expanded="false"');
    expect(collapsedHtml).not.toContain("Поставь музыку на паузу");

    const searchHtml = renderToStaticMarkup(<VoiceCommands initialQuery="скриншот" />);
    expect(searchHtml).toContain('aria-expanded="true"');
    expect(searchHtml).toContain("Сделай скриншот");
    expect(searchHtml).toContain("работает без ИИ");
  });
});
