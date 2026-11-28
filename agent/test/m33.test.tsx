import { describe, expect, it } from "vitest";
import { renderToStaticMarkup } from "react-dom/server";
import { setLanguage } from "@decka/shared";
import ReplyVoiceBlock from "../src/tabs/ReplyVoiceBlock";
import { clampReplySpeed, DEFAULT_REPLY_SPEED, findPiperVoice, PIPER_VOICES } from "../src/tabs/voiceM33Types";
import { VOICE_COMMAND_GROUPS } from "../src/tabs/voiceCommandsData";

describe("M33 T33.1b ReplyVoiceBlock and voice help", () => {
  it("renders Piper engine without downloaded voice and default speed 1.05", () => {
    setLanguage("ru");
    const html = renderToStaticMarkup(<ReplyVoiceBlock />);

    expect(html).toContain("Голос ответа");
    expect(html).toContain("Нейросетевой (Piper)");
    expect(html).toContain("Голос Windows");
    expect(html).toContain("Не скачан");
    expect(html).toContain("Скачать");
    expect(html).toContain("1.05×");
    expect(html).toContain('min="0.8"');
    expect(html).toContain('max="1.3"');
  });

  it("renders Windows engine mode without Piper voice controls", () => {
    setLanguage("ru");
    const html = renderToStaticMarkup(<ReplyVoiceBlock initialConfig={{ engine: "windows" }} />);

    expect(html).toContain("Встроенный синтез Windows без загрузки моделей");
    expect(html).not.toContain("Не скачан");
    expect(html).not.toContain(">Скачать<");
  });

  it("renders Listen and Remove actions when selected Piper voice is downloaded", () => {
    setLanguage("ru");
    const html = renderToStaticMarkup(
      <ReplyVoiceBlock
        initialConfig={{
          engine: "piper",
          voiceId: "ru_RU-irina-medium",
          speed: 1.15,
          downloadedIds: ["ru_RU-irina-medium"],
        }}
      />,
    );

    expect(html).toContain("Скачан");
    expect(html).toContain("Послушать");
    expect(html).toContain("Удалить");
    expect(html).toContain("1.15×");
  });

  it("renders downloading progress percentage cleanly", () => {
    setLanguage("en");
    const html = renderToStaticMarkup(<ReplyVoiceBlock initialDownloadPct={45} />);

    expect(html).toContain("Downloading… 45%");
    expect(html).toContain("Neural (Piper)");
  });

  it("clamps speed within 0.8..1.3 and resolves Piper voices with sizes", () => {
    expect(DEFAULT_REPLY_SPEED).toBe(1.05);
    expect(clampReplySpeed(0.4)).toBe(0.8);
    expect(clampReplySpeed(1.9)).toBe(1.3);
    expect(clampReplySpeed(Number.NaN)).toBe(1.05);
    expect(PIPER_VOICES.length).toBeGreaterThanOrEqual(10);
    expect(findPiperVoice("en_US-lessac-medium").sizeMb).toBe(63);
  });

  it("includes faster/slower/change_voice examples in the read_aloud group of What can I say", () => {
    const group = VOICE_COMMAND_GROUPS.find((g) => g.id === "read_aloud");
    expect(group).toBeTruthy();
    const ruTexts = group!.examples.map((e) => e.ru).join(" ");
    expect(ruTexts).toContain("Говори быстрее");
    expect(ruTexts).toContain("Смени голос");
  });
});
