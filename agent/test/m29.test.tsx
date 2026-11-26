import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { CATALOG, setLanguage } from "@decka/shared";
import VoiceSection from "../src/tabs/VoiceSection";

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
