import { describe, expect, it } from "vitest";
import { renderToString } from "react-dom/server";
import { setLanguage } from "@decka/shared";
import Island, { chipIconKind, expandedStatusText, hasCodeBlock, hasVoiceContent, visibleChips } from "../src/island/Island";
import DictationOrb from "../src/island/DictationOrb";
import { compactWidth, currentWraps } from "../src/island/compactWidth";
import ParticlesOrb, { particleCountForSize } from "../src/island/ParticlesOrb";
import { compactVoiceText, orbStateFromIsland } from "../src/island/islandTypes";

describe("T26.5b ParticlesOrb & Island", () => {
  it("computes particle counts per size rule (480 solo, 160 small)", () => {
    expect(particleCountForSize(140)).toBe(480);
    expect(particleCountForSize(72)).toBe(160);
    expect(particleCountForSize(32)).toBe(160);
  });

  it("T28.5b: compact width is fixed at 440 px for song and wraps current onto lines 2-3 when longer", () => {
    const m = (text: string) => text.length * 8;
    expect(compactWidth("Hi", "yo", m)).toBe(440);
    expect(compactWidth("x".repeat(40), "y", m)).toBe(440);
    expect(compactWidth("x", "y".repeat(200), m)).toBe(440);
    expect(currentWraps("x".repeat(50), m)).toBe(true);
    expect(currentWraps("x".repeat(30), m)).toBe(false);
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
    expect(expandedStatusText("loading", { etaSec: 12 })).toBe("Загружаю модель… ~12 с");
    // The seconds belong to the load only.
    expect(expandedStatusText("listening", { etaSec: 12 })).toBe("Слушаю…");

    // T34.2: while something is read aloud the island says how to end it.
    const reading = (stopHint: "voice" | "esc") => renderToString(<Island event={{ mode: "expanded", voice: { active: true, phase: "thinking", stopHint } }} />);
    expect(reading("voice")).toContain("Остановить: «стоп» или Esc");
    expect(reading("esc")).toContain("Остановить: Esc");

    // T34.3: a result that was not pasted: the text, and what to do with it.
    const card = (pasteHint: "copied" | "check") => renderToString(<Island event={{ mode: "expanded", voice: { active: false, phase: "done", reply: "The translation", pasteHint } }} />);
    expect(card("copied")).toContain("Скопировано — вставь Ctrl+V");
    expect(card("copied")).toContain("island-reply is-paste");
    expect(card("check")).toContain("Если текст не изменился — вставь Ctrl+V");
    expect(card("copied") + card("check")).not.toContain("е удалось вставить");

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

  it("T28.5b: renders 3 centered lines (prev/current/next) and hides next when long current wraps to 2 lines", () => {
    const threeLinesHtml = renderToString(
      <Island
        event={{
          mode: "compact",
          playing: true,
          lyrics: {
            playing: true,
            prev: "Prev line",
            current: "Current line",
            next: "Next line",
          } as never,
        }}
      />,
    );
    expect(threeLinesHtml).toContain("--cw:440px");
    expect(threeLinesHtml).toContain("island-lyric-prev");
    expect(threeLinesHtml).toContain("Prev line");
    expect(threeLinesHtml).toContain("Current line");
    expect(threeLinesHtml).toContain("Next line");
    expect(threeLinesHtml).not.toContain("is-two-line");

    const wrappedHtml = renderToString(
      <Island
        event={{
          mode: "compact",
          playing: true,
          lyrics: {
            playing: true,
            prev: "Prev line",
            current: "This current lyric line is definitely much longer than one compact line width",
            next: "Next hidden",
          } as never,
        }}
      />,
    );
    expect(wrappedHtml).toContain("is-two-line");
    expect(wrappedHtml).toContain("Prev line");
    expect(wrappedHtml).not.toContain("Next hidden");
  });

  it("renders M29 Yes/No confirm buttons inside expanded panel, 10px notch tab bar with musicLevel, and 24px DictationOrb", () => {
    const confirmHtml = renderToString(
      <Island
        event={{
          mode: "expanded",
          voice: {
            active: true,
            phase: "confirm",
            level: 0,
            heard: "Перезагрузить компьютер",
            confirmId: "c1",
            confirm: { text: "Перезагрузить компьютер?", key: "voice.confirm.action" },
          },
        }}
        onConfirm={() => {}}
      />,
    );
    expect(confirmHtml).toContain("island-confirm-row");
    expect(confirmHtml).toContain("island-confirm-btn");
    expect(confirmHtml).toContain("is-yes");
    expect(confirmHtml).toContain("is-no");

    const tabHtml = renderToString(
      <Island
        event={{
          mode: "tab",
          playing: true,
          musicLevel: 0.72,
          track: { title: "Midnight City", artist: "M83" },
        }}
        barHeight={10}
        barPosition="center"
      />,
    );
    expect(tabHtml).toContain("island-tab-zone");
    expect(tabHtml).toContain("island-tab-bar");
    expect(tabHtml).toContain("island-tab-meter");
    expect(tabHtml).toContain("height:10px");

    const orbHtml = renderToString(
      <DictationOrb state="listening" level={0.6} error="Нет активного поля ввода" x={100} y={200} />,
    );
    expect(orbHtml).toContain("island-dictation-wrap");
    expect(orbHtml).toContain("island-dictation-bubble");
    expect(orbHtml).toContain("Нет активного поля ввода");
  });

  it("T30.1b: renders recording indicator in tab/compact/expanded and memory-top list with Close buttons", () => {
    setLanguage("ru");
    const recTabHtml = renderToString(
      <Island event={{ mode: "tab" }} recording={{ active: true, elapsedSec: 125 }} />,
    );
    expect(recTabHtml).toContain("island-tab-rec");
    expect(recTabHtml).toContain("island-rec-dot");
    expect(recTabHtml).toContain("2:05");

    const memHtml = renderToString(
      <Island
        event={{ mode: "expanded", voice: { active: true, phase: "done", level: 0, heard: "Что ест память?" } }}
        recording={{ active: true, elapsedSec: 65 }}
        memoryProcesses={[
          { pid: 101, name: "chrome.exe", mb: 2450 },
          { pid: 202, name: "Code.exe", mb: 1320 },
        ]}
        onCloseProcess={() => {}}
      />,
    );
    expect(memHtml).toContain("island-rec-pill");
    expect(memHtml).toContain("1:05");
    expect(memHtml).toContain("island-mem-list");
    expect(memHtml).toContain("chrome.exe");
    expect(memHtml).toContain("2450");
    // The unit is a word of the language.
    expect(memHtml).toContain("2450 МБ");
    expect(memHtml).toContain("Закрыть");
  });

  it("T30.R: the recording can be stopped and the memory note has its button", () => {
    setLanguage("ru");
    const noop = () => {};
    const compact = renderToString(<Island event={{ mode: "compact" }} recording={{ active: true, elapsedSec: 5 }} onStopRecording={noop} />);
    expect(compact).toContain("island-rec-stop");
    const listening = renderToString(<Island event={{ mode: "expanded", voice: { active: true, phase: "listening" } }} recording={{ active: true, elapsedSec: 5 }} onStopRecording={noop} />);
    expect(listening).toContain("island-rec-stop");
    expect(listening).toContain("0:05");
    const note = renderToString(<Island event={{ mode: "expanded", voice: { active: false, phase: "done", reply: "Память занята", actions: ["memory"] } }} onAction={noop} />);
    expect(note).toContain("Показать, что ест");
  });

  it("T31.2b: renders error card with red left stripe class, retry action, and 400ms data-tooltip", () => {
    setLanguage("ru");
    const errHtml = renderToString(
      <Island
        event={{
          mode: "expanded",
          voice: { active: true, phase: "error", level: 0, heard: "Открой", reply: "Не удалось распознать" },
        }}
        onAction={() => {}}
      />,
    );
    expect(errHtml).toContain("island-error-card");
    expect(errHtml).toContain("Не удалось распознать");

    const actionHtml = renderToString(
      <Island
        event={{
          mode: "expanded",
          voice: { active: true, phase: "done", level: 0, heard: "Привет", reply: "Готово", actions: ["copy", "again"] },
        }}
        onAction={() => {}}
      />,
    );
    expect(actionHtml).toContain("data-tooltip=");
    expect(actionHtml).toContain("island-reply");
  });
});
