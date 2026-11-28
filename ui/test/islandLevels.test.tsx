// T32.F: the levels of the island move the meter and the orb without a new state.
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, test } from "vitest";
import Island from "../src/island/Island";
import { createLevels, meterScale } from "../src/island/levels";

describe("island levels", () => {
  test("a change reaches those who listen until they stop", () => {
    const levels = createLevels();
    const seen: number[] = [];
    const stop = levels.subscribe(() => seen.push(levels.get().music));
    levels.set({ music: 0.4, voice: 0 });
    stop();
    levels.set({ music: 0.9, voice: 0 });
    expect(seen).toEqual([0.4]);
    expect(levels.get().music).toBe(0.9);
  });

  test("the meter of the bar is scaled, never wider than the bar, and thin at silence", () => {
    expect([meterScale(undefined), meterScale(0), meterScale(0.5), meterScale(2)]).toEqual([0, 0.12, 0.5, 1]);
  });

  test("the bar has no canvas: only a meter with a transform that follows the live level", () => {
    const levels = createLevels();
    levels.set({ music: 0.7, voice: 0 });
    const html = renderToStaticMarkup(<Island event={{ mode: "tab", playing: true }} levels={levels} />);
    expect(html).toContain("scaleX(0.7)");
    expect(html).not.toContain("<canvas");
  });
});
