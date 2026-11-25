import { describe, expect, it } from "vitest";
import { renderToString } from "react-dom/server";
import { setLanguage } from "@decka/shared";
import Island, { expandedStatusText, hasVoiceContent } from "../src/island/Island";
import ParticlesOrb, { particleCountForSize } from "../src/island/ParticlesOrb";
import { orbStateFromIsland } from "../src/island/islandTypes";

describe("T26.5b ParticlesOrb & Island", () => {
  it("computes particle counts per size rule (720 at 168, >= 140)", () => {
    expect(particleCountForSize(168)).toBe(720);
    expect(particleCountForSize(120)).toBe(Math.round(720 * Math.pow(120 / 168, 2)));
    expect(particleCountForSize(32)).toBe(140);
    expect(particleCountForSize(24)).toBe(140);
  });

  it("maps island voice states and errors to orb state", () => {
    expect(orbStateFromIsland({ mode: "compact" })).toEqual({ orbState: "idle", isError: false });
    expect(orbStateFromIsland({ mode: "expanded", voice: { active: true, phase: "loading" } })).toEqual({
      orbState: "connecting",
      isError: false,
    });
    expect(orbStateFromIsland({ mode: "expanded", voice: { active: true, phase: "listening", level: 0.6 } })).toEqual({
      orbState: "listening",
      isError: false,
    });
    expect(orbStateFromIsland({ mode: "expanded", voice: { active: true, phase: "thinking", slow: { etaSec: 10 } } })).toEqual({
      orbState: "thinking",
      isError: false,
    });
    expect(orbStateFromIsland({ mode: "expanded", voice: { active: true, phase: "speaking" } })).toEqual({
      orbState: "speaking",
      isError: false,
    });
    expect(orbStateFromIsland({ mode: "expanded", voice: { active: false, phase: "error", reply: "Failed" } })).toEqual({
      orbState: "idle",
      isError: true,
    });
  });

  it("distinguishes solo (260x230, 140px orb) vs content (520, 72px orb) expanded layouts", () => {
    setLanguage("ru");
    expect(hasVoiceContent({ mode: "expanded", voice: { active: true, phase: "listening" } })).toBe(false);
    expect(hasVoiceContent({ mode: "expanded", voice: { active: true, phase: "listening", heard: "Привет" } })).toBe(true);
    expect(expandedStatusText("listening")).toBe("Слушаю…");
    expect(expandedStatusText("loading")).toBe("Подключено");

    const soloHtml = renderToString(
      <Island event={{ mode: "expanded", voice: { active: true, phase: "listening", level: 0.5 } }} />,
    );
    expect(soloHtml).toContain("island-expanded is-solo");
    expect(soloHtml).toContain("island-orb-slot is-solo");
    expect(soloHtml).toContain("width:140px;height:140px");
    expect(soloHtml).toContain("Слушаю…");
    expect(soloHtml).not.toContain("island-content-col");

    const contentHtml = renderToString(
      <Island
        event={{
          mode: "expanded",
          voice: { active: true, phase: "thinking", heard: "Что на экране?", slow: { etaSec: 12, elapsedSec: 4 } },
        }}
      />,
    );
    expect(contentHtml).toContain("island-expanded is-content");
    expect(contentHtml).toContain("island-orb-slot is-content");
    expect(contentHtml).toContain("width:72px;height:72px");
    expect(contentHtml).toContain("island-content-col");
    expect(contentHtml).toContain("Что на экране?");

    const midHtml = renderToString(
      <Island
        transitionPreview={0.5}
        event={{
          mode: "expanded",
          voice: { active: true, phase: "done", heard: "Hello", reply: "World" },
        }}
      />,
    );
    expect(midHtml).toContain("is-transitioning");
    expect(midHtml).toContain("width:488px");
    expect(midHtml).toContain("width:140px;height:140px");
  });

  it("renders hidden, tab, compact lyrics, and expanded action chips via t()", () => {
    setLanguage("en");
    expect(renderToString(<Island event={{ mode: "hidden" }} />)).toBe("");

    const tabHtml = renderToString(<Island event={{ mode: "tab" }} />);
    expect(tabHtml).toContain("island-tab-bar");

    const compactLyricsHtml = renderToString(
      <Island
        event={{ mode: "compact", lyrics: { playing: true, current: "Line 1", next: "Line 2" } }}
      />,
    );
    expect(compactLyricsHtml).toContain("island-compact two-lines is-lyrics");
    expect(compactLyricsHtml).toContain("Line 1");
    expect(compactLyricsHtml).toContain("Line 2");

    const expandedHtml = renderToString(
      <Island
        event={{
          mode: "expanded",
          voice: {
            active: true,
            phase: "thinking",
            heard: "what is on screen?",
            reply: "Answer text",
            slow: { etaSec: 12, elapsedSec: 4 },
            actions: ["copy", "insert", "speak", "again", "lang:en"],
            chips: [{ label: "Translate", command: "translate" }],
          },
        }}
      />,
    );
    expect(expandedHtml).toContain("island-expanded is-content");
    expect(expandedHtml).toContain("island-slow-track");
    expect(expandedHtml).toContain("Answer text");
    expect(expandedHtml).toContain("Translate");
    expect(expandedHtml).toContain("particles-orb");

    const orbHtml = renderToString(<ParticlesOrb state="listening" size={120} level={0.5} />);
    expect(orbHtml).toContain('role="img"');
  });
});
