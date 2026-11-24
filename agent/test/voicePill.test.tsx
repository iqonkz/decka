import { describe, expect, test } from "vitest";
import { renderToString } from "react-dom/server";
import VoicePill, { activityOf, waveHeights } from "../src/VoicePill";

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

describe("the pill of dictation and of a look at the screen (DATA_MODEL.md 13.7)", () => {
  test("dictation says Dictating with a microphone while it listens, recognises and thinks", () => {
    for (const state of ["listening", "recognizing", "thinking"] as const) {
      const html = renderToString(<VoicePill initialData={{ state, activity: "dictate" }} />);
      expect(html, state).toContain("Dictating");
      expect(html, state).toContain("voice-activity-icon");
      expect(html, state).toContain("M12 19v3"); // the microphone
    }
    expect(renderToString(<VoicePill initialData={{ state: "listening", activity: "dictate" }} />)).toContain("voice-stop");
  });

  test("the screen help says Looking at the screen with an eye while it works", () => {
    const html = renderToString(<VoicePill initialData={{ state: "thinking", activity: "screen" }} />);
    expect(html).toContain("Looking at the screen");
    expect(html).toContain("M18.944 12.33"); // scan-eye
    expect(html).not.toContain("Thinking");
  });

  test("words already heard, the end of the request and requests without an activity keep the ordinary labels", () => {
    expect(activityOf({ state: "listening", heard: "hello", activity: "dictate" })).not.toBeNull();
    expect(renderToString(<VoicePill initialData={{ state: "listening", heard: "hello world", activity: "dictate" }} />)).toContain("hello world");
    expect(renderToString(<VoicePill initialData={{ state: "done", reply: "Pasted", activity: "dictate" }} />)).toContain("Pasted");
    expect(activityOf({ state: "done", activity: "screen" })).toBeNull();
    expect(activityOf({ state: "thinking" })).toBeNull();
    expect(activityOf({ state: "listening", activity: "screen" })).toBeNull();
    expect(renderToString(<VoicePill initialData={{ state: "thinking" }} />)).toContain("Thinking");
  });
});
