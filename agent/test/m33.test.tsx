import { describe, expect, it } from "vitest";
import { renderToStaticMarkup } from "react-dom/server";
import { setLanguage } from "@decka/shared";
import ReplyVoiceBlock from "../src/tabs/ReplyVoiceBlock";
import { VOICE_COMMAND_GROUPS } from "../src/tabs/voiceCommandsData";

describe("M33 reply voice", () => {
  it("shows the speed the PC keeps, between 0.80 and 1.30", () => {
    setLanguage("ru");
    const html = renderToStaticMarkup(<ReplyVoiceBlock percent={105} onChange={() => {}} />);
    expect(html).toContain("Голос ответа");
    expect(html).toContain("1.05×");
    expect(html).toContain('min="80"');
    expect(html).toContain('max="130"');
    expect(html).not.toContain("Piper");
    // The usual speed has nothing to go back to.
    expect(html).not.toContain("<button");
  });

  it("offers the usual speed again when another one is set", () => {
    setLanguage("en");
    const html = renderToStaticMarkup(<ReplyVoiceBlock percent={125} onChange={() => {}} />);
    expect(html).toContain("1.25×");
    expect(html).toContain("Reset (1.05)");
  });

  it("lists faster, slower and another voice in What can I say", () => {
    const said = VOICE_COMMAND_GROUPS.find((g) => g.id === "read_aloud")!.examples.map((e) => e.ru).join(" ");
    expect(said).toContain("Говори быстрее");
    expect(said).toContain("Смени голос");
  });
});
