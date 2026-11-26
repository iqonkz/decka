import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { CATALOG, setLanguage, type ConfigFile } from "@decka/shared";
import VoiceSection from "../src/tabs/VoiceSection";
import VoiceCommands from "../src/tabs/VoiceCommands";
import ScreenHelpTab from "../src/tabs/ScreenHelpTab";
import { buildIntegrationsList } from "../src/tabs/integrationsCatalog";
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

describe("M29.13b Screen assist page & Integrations completeness", () => {
  it("renders ScreenHelpTab with toggle, pack status, privacy rules, and examples", () => {
    setLanguage("ru");
    const html = renderToStaticMarkup(
      <ScreenHelpTab initialEnabled={true} initialPackReady={false} initialPackSize="1.8 GB" />,
    );
    expect(html).toContain("Помощь по экрану");
    expect(html).toContain("Пакет анализа экрана");
    expect(html).toContain("Скачать");
    expect(html).toContain("Что читается");
    expect(html).toContain("Что сейчас на экране?");
  });

  it("provides an integration card for every config.json integration section", () => {
    setLanguage("ru");
    const sampleConfig: ConfigFile = {
      version: 1,
      agentName: "PC",
      port: 9247,
      bind: "0.0.0.0",
      apiToken: "dk_test",
      autostart: false,
      usb: { enabled: false, adbPath: "adb" },
      collectors: { sys: { enabled: true, intervalSec: 2 }, checks: [] },
      obs: { enabled: true, port: 4455 },
      twitch: { enabled: false, clientId: "" },
      youtube: { enabled: false, clientId: "" },
      mail: { accounts: [] },
      windowsNotifications: { enabled: false, exclude: [] },
      weather: { enabled: false, place: "", lat: 0, lon: 0, units: "metric" },
      ai: { claude: { enabled: false, approvals: true, timeoutSec: 60 }, codex: { enabled: false } },
    };
    const cards = buildIntegrationsList(sampleConfig, 0, true);
    const ids = new Set(cards.map((c) => c.id));
    for (const required of ["obs", "twitch", "youtube", "mail", "winnotif", "weather", "checks", "ai", "lyrics", "bluetooth", "webhook", "api_token"]) {
      expect(ids.has(required), `Missing integration card: ${required}`).toBe(true);
    }
    expect(cards.find((c) => c.id === "obs")?.connected).toBe(true);
    expect(cards.find((c) => c.id === "lyrics")?.connected).toBe(true);
    expect(cards.find((c) => c.id === "twitch")?.connected).toBe(false);
  });
});
