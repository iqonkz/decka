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

  it("T29.2: Yes and No of a question are inside the panel, and a download shows how far it is", () => {
    const ask = renderToString(<Island event={{ mode: "expanded", voice: { active: true, phase: "confirm", reply: "Скачать пакет?", confirmId: "ab" } }} />);
    const panel = ask.indexOf("island-panel");
    expect(panel).toBeGreaterThan(-1);
    expect(ask.indexOf("island-confirm-btn")).toBeGreaterThan(panel);
    expect(ask).toContain("Да");
    expect(ask).toContain("Нет");
    const done = renderToString(<Island event={{ mode: "expanded", voice: { active: false, phase: "done", reply: "Готово", confirmId: "ab" } }} />);
    expect(done).not.toContain("island-confirm");
    const loading = renderToString(<Island event={{ mode: "expanded", voice: { active: true, phase: "thinking", download: { pct: 42 } } }} />);
    expect(loading).toContain("42%");
    expect(loading).toContain("width:42%");
  });

  it("a phrase that is not a command yet is shown as it was heard", () => {
    const html = renderToString(<Island event={{ mode: "expanded", voice: { active: true, phase: "thinking", heard: "что на экране" } }} />);
    expect(html).toContain("что на экране");
    expect(html).not.toContain("Услышал");
    expect(html).not.toContain("Выполняю");
  });
});
