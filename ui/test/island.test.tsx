import { describe, expect, it } from "vitest";
import { renderToString } from "react-dom/server";
import { setLanguage } from "@decka/shared";
import Island, { chipIconKind, expandedStatusText, hasCodeBlock, hasVoiceContent, visibleChips } from "../src/island/Island";
import { compactWidth, currentWraps } from "../src/island/compactWidth";
import ParticlesOrb, { particleCountForSize } from "../src/island/ParticlesOrb";
import { compactVoiceText, orbStateFromIsland } from "../src/island/islandTypes";

describe("T26.5b ParticlesOrb & Island", () => {
  it("computes particle counts per size rule (480 solo, 160 small)", () => {
    expect(particleCountForSize(140)).toBe(480);
    expect(particleCountForSize(72)).toBe(160);
    expect(particleCountForSize(32)).toBe(160);
  });

  it("T26.F-8/14: compact width is fixed at 520 px for song and long current lines wrap", () => {
    const m = (text: string) => text.length * 8;
    expect(compactWidth("Hi", "yo", m)).toBe(520);
    expect(compactWidth("x".repeat(40), "y", m)).toBe(520);
    expect(compactWidth("x", "y".repeat(200), m)).toBe(520);
    expect(currentWraps("x".repeat(60), m)).toBe(true);
    expect(currentWraps("x".repeat(40), m)).toBe(false);
  });

  it("T26.F.12: a song without a current line, with the voice idle, is not listening", () => {
    const song = { mode: "compact" as const, playing: true, lyrics: { playing: true } };
    // The song moves the orb like speech (T26.F.11), but it is never "listening".
    expect(orbStateFromIsland(song).orbState).toBe("speaking");
    expect(compactVoiceText(undefined, "Listening...")).toBe("");
    expect(compactVoiceText({ active: false, phase: "idle" }, "Listening...")).toBe("");
    expect(orbStateFromIsland({ ...song, voice: { active: false, phase: "listening" } }).orbState).not.toBe("listening");
    expect(orbStateFromIsland({ mode: "compact", voice: { active: false, phase: "listening" } }).orbState).toBe("idle");
    expect(orbStateFromIsland({ ...song, voice: { active: true, phase: "listening" } }).orbState).toBe("listening");
    expect(compactVoiceText({ active: true, phase: "listening" }, "Listening...")).toBe("Listening...");
  });

  it("T26.F.9: the copy-code chip needs a code block in the answer", () => {
    const chips = [{ label: "Copy code", command: "copy_code" }, { label: "Translate", command: "translate" }];
    expect(hasCodeBlock("text\n```js\nlet a = 1;\n```")).toBe(true);
    expect(hasCodeBlock("no code here")).toBe(false);
    expect(hasCodeBlock("one ``` fence only")).toBe(false);
    expect(visibleChips(chips, "plain answer").map((c) => c.command)).toEqual(["translate"]);
    expect(visibleChips(chips, "```\nx\n```").map((c) => c.command)).toEqual(["copy_code", "translate"]);
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

    // T26.F.9: order heard (no quotes) -> reply -> chips -> icon action bar + feedback states.
    const idxHeard = expandedHtml.indexOf("island-heard");
    const idxReply = expandedHtml.indexOf("island-reply");
    const idxChips = expandedHtml.indexOf("island-chips");
    const idxActions = expandedHtml.indexOf("island-actions");
    expect(idxHeard).toBeGreaterThan(-1);
    expect(idxHeard).toBeLessThan(idxReply);
    expect(idxReply).toBeLessThan(idxChips);
    expect(idxChips).toBeLessThan(idxActions);
    expect(expandedHtml).not.toContain("“what is on screen?”");
    expect(expandedHtml).toContain('aria-label="Copy"');
    expect(expandedHtml).toContain('aria-label="Insert"');
    expect(expandedHtml).toContain('aria-label="Speak"');
    expect(expandedHtml).toContain('aria-label="Try again"');
    expect(expandedHtml).toContain("island-actions-sep");
    expect(expandedHtml).toContain(">EN<");
    expect(expandedHtml).toContain("island-icon-spin");

    expect(chipIconKind({ label: "Copy snippet", command: "copy" })).toBe("code");
    expect(chipIconKind({ label: "Перевести", command: "translate" })).toBe("translate");
    expect(chipIconKind({ label: "Reply", command: "reply" })).toBe("reply");
    expect(chipIconKind({ label: "What error?", command: "screen_ask" })).toBe("help");

    const copiedAndSpeakingHtml = renderToString(
      <Island
        copiedPreview
        langMenuOpenPreview
        hoveredAction="insert"
        event={{
          mode: "expanded",
          voice: {
            active: true,
            phase: "speaking",
            heard: "hello",
            reply: "Hi there",
            actions: ["copy", "insert", "speak", "again", "lang"],
          },
        }}
      />,
    );
    expect(copiedAndSpeakingHtml).toContain("is-copied");
    expect(copiedAndSpeakingHtml).toContain('aria-label="Copied"');
    expect(copiedAndSpeakingHtml).toContain('aria-label="Stop speaking"');
    expect(copiedAndSpeakingHtml).toContain("is-hovered");
    expect(copiedAndSpeakingHtml).toContain("island-lang-menu");
    expect(copiedAndSpeakingHtml).toContain("is-current");

    const orbHtml = renderToString(<ParticlesOrb state="listening" size={120} level={0.5} />);
    expect(orbHtml).toContain('role="img"');
  });

  it("T26.F.11/14: drives 48px compact orb (160 pts) with musicLevel and renders 3 hover controls over orb", () => {
    setLanguage("en");
    expect(orbStateFromIsland({ mode: "compact", playing: true, musicLevel: 0.7 })).toEqual({
      orbState: "speaking",
      isError: false,
    });
    expect(orbStateFromIsland({ mode: "compact", playing: false, musicLevel: 0.7 })).toEqual({
      orbState: "idle",
      isError: false,
    });

    const playingHtml = renderToString(
      <Island
        hovered
        event={{
          mode: "compact",
          playing: true,
          musicLevel: 0.65,
          muted: false,
          track: { title: "Midnight City", artist: "M83" },
          lyrics: { playing: true, current: "First line", next: "Second line" },
        }}
      />,
    );
    expect(playingHtml).toContain('data-particles="160"');
    expect(playingHtml).toContain("width:48px;height:48px");
    expect(playingHtml).toContain("island-compact-left");
    expect(playingHtml).toContain("island-compact-controls");
    expect(playingHtml).toContain("is-play");
    expect(playingHtml).not.toContain("is-prev");
    expect(playingHtml).toContain("is-next");
    expect(playingHtml).not.toContain('aria-label="Previous track"');
    expect(playingHtml).toContain('aria-label="Pause"');
    expect(playingHtml).toContain('aria-label="Next track"');
    expect(playingHtml).not.toContain('aria-label="Mute"');

    setLanguage("ru");
    const mutedPausedHtml = renderToString(
      <Island
        event={{
          mode: "compact",
          playing: false,
          muted: true,
          track: { title: "Midnight City", artist: "M83" },
          lyrics: { playing: false, current: "Строка" },
        }}
      />,
    );
    expect(mutedPausedHtml).toContain('data-particles="160"');
    expect(mutedPausedHtml).toContain('aria-label="Воспроизвести"');
    expect(mutedPausedHtml).not.toContain('aria-label="Включить звук"');
  });

  it("T26.F.12: renders countdown, unknown waiting dots, and track-only fallback without Listening", () => {
    setLanguage("ru");
    const countdownHtml = renderToString(
      <Island
        event={{
          mode: "compact",
          playing: true,
          track: { title: "Midnight City", artist: "M83" },
          lyrics: { playing: true, firstLineInMs: 12000 },
        }}
      />,
    );
    expect(countdownHtml).toContain("island-compact two-lines is-lyrics is-song");
    expect(countdownHtml).toContain("island-song-icon");
    expect(countdownHtml).toContain("Midnight City — M83");
    expect(countdownHtml).toContain("Текст через 0:12");
    expect(countdownHtml).not.toContain("Слушаю");

    setLanguage("en");
    const unknownHtml = renderToString(
      <Island
        event={{
          mode: "compact",
          playing: true,
          track: { title: "Midnight City", artist: "M83" },
          lyrics: { playing: true, status: "loading" },
        }}
      />,
    );
    expect(unknownHtml).toContain("Midnight City — M83");
    expect(unknownHtml).toContain("island-lyrics-dots");
    expect(unknownHtml.match(/class="island-lyrics-dot"/g)?.length).toBe(3);
    expect(unknownHtml).not.toContain("Listening");

    const noLyricsHtml = renderToString(
      <Island
        event={{
          mode: "compact",
          playing: true,
          track: { title: "Midnight City", artist: "M83" },
        }}
      />,
    );
    expect(noLyricsHtml).toContain("island-song-icon");
    expect(noLyricsHtml).toContain(">Midnight City<");
    expect(noLyricsHtml).toContain(">M83<");
    expect(noLyricsHtml).not.toContain("Listening");

    const emptySongHtml = renderToString(
      <Island event={{ mode: "compact", lyrics: { playing: true } }} />,
    );
    expect(emptySongHtml).not.toContain("Listening");
  });
});
