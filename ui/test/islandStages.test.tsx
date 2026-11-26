// T28.3: a command shows its stages on the island: "Heard" -> "Understood" -> "Doing it".
import { renderToString } from "react-dom/server";
import { beforeEach, describe, expect, it } from "vitest";
import { setLanguage } from "@decka/shared";
import { Island } from "../src";

describe("the previous line of a song (T28.5b)", () => {
  it("is what the PC says it is: none on a new track or at the first line", () => {
    const song = (title: string, lyrics: { current: string; prev?: string }) =>
      renderToString(<Island event={{ mode: "compact", playing: true, track: { title, artist: "A" }, lyrics: { playing: true, ...lyrics } }} />);
    expect(song("One", { current: "second line", prev: "first line" })).toContain("first line");
    const next = song("Two", { current: "opening line" });
    expect(next).toContain('island-lyric-prev"></div>');
    expect(next).not.toContain("first line");
  });
});

describe("the stages of a command (T28.3)", () => {
  beforeEach(() => setLanguage("ru"));

  it("heard, understood and doing it, in this order", () => {
    const html = renderToString(
      <Island
        event={{
          mode: "expanded",
          voice: { active: true, phase: "thinking", heard: "громче", understood: { text: "изменить громкость" }, executing: true },
        }}
      />,
    );
    const heard = html.indexOf("Услышал: «громче»");
    const understood = html.indexOf("Понял: изменить громкость");
    const doing = html.indexOf("Выполняю…");
    expect(heard).toBeGreaterThan(-1);
    expect(understood).toBeGreaterThan(heard);
    expect(doing).toBeGreaterThan(understood);
  });

  it("a phrase that is not a command yet is shown as it was heard", () => {
    const html = renderToString(<Island event={{ mode: "expanded", voice: { active: true, phase: "thinking", heard: "что на экране" } }} />);
    expect(html).toContain("что на экране");
    expect(html).not.toContain("Услышал");
    expect(html).not.toContain("Выполняю");
  });
});
