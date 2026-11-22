import { describe, expect, test } from "vitest";
import { renderToString } from "react-dom/server";
import VoicePill, { waveHeights } from "../src/VoicePill";

describe("VoicePill component", () => {
  test("the wave follows the level: middle tallest, silence lies at 4 px", () => {
    expect(waveHeights(0)).toEqual([4, 4, 4, 4, 4]);
    expect(waveHeights(1)).toEqual([14, 20, 24, 20, 14]);
    expect(waveHeights(NaN)).toEqual([4, 4, 4, 4, 4]);
    expect(waveHeights(5)).toEqual(waveHeights(1));
  });

  test("the stop button is only there while listening", () => {
    expect(renderToString(<VoicePill initialData={{ state: "listening" }} />)).toContain("voice-stop");
    expect(renderToString(<VoicePill initialData={{ state: "thinking" }} />)).not.toContain("voice-stop");
    expect(renderToString(<VoicePill initialData={{ state: "done", reply: "ok" }} />)).not.toContain("voice-stop");
  });

  test("renders listening state", () => {
    const html = renderToString(<VoicePill initialData={{ state: "listening", level: 0.5 }} />);
    expect(html).toContain("voice-pill");
    expect(html).toContain("Listening…");
  });

  test("renders heard text in listening state", () => {
    const html = renderToString(<VoicePill initialData={{ state: "listening", heard: "open calculator" }} />);
    expect(html).toContain("open calculator");
  });

  test("renders thinking state", () => {
    const html = renderToString(<VoicePill initialData={{ state: "thinking" }} />);
    expect(html).toContain("Thinking");
  });

  test("renders done state with reply", () => {
    const html = renderToString(<VoicePill initialData={{ state: "done", reply: "Calculator opened" }} />);
    expect(html).toContain("Calculator opened");
  });

  test("renders confirm state with actions", () => {
    const html = renderToString(
      <VoicePill initialData={{ state: "confirm", confirm: { text: "Shut down PC?" } }} />
    );
    expect(html).toContain("Shut down PC?");
    expect(html).toContain("Yes");
    expect(html).toContain("No");
  });
});
