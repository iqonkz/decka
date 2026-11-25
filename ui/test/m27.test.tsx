import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { setLanguage } from "@decka/shared";
import { HighlightOverlay, computeBubblePlacement, GuidePanel } from "../src";

describe("M27 visual components (T27.2b)", () => {
  it("computes bubble placement inside viewport bounds on all edges", () => {
    const vp = { width: 1280, height: 720 };
    const topBox = computeBubblePlacement({ x: 20, y: 20, width: 120, height: 36 }, vp);
    expect(topBox.side).toBe("bottom");
    expect(topBox.left).toBeGreaterThanOrEqual(8);
    expect(topBox.top).toBeGreaterThanOrEqual(20 + 36);

    const bottomBox = computeBubblePlacement({ x: 400, y: 670, width: 120, height: 36 }, vp);
    expect(bottomBox.side).toBe("top");
    expect(bottomBox.top).toBeLessThan(670);

    const rightBox = computeBubblePlacement({ x: 1200, y: 120, width: 70, height: 30 }, vp);
    expect(rightBox.left + 280).toBeLessThanOrEqual(vp.width - 8);
  });

  it("renders HighlightOverlay frame and bubble and respects reducedMotion", () => {
    setLanguage("ru");
    const html = renderToStaticMarkup(
      <HighlightOverlay
        contained
        reducedMotion
        viewport={{ width: 1000, height: 600 }}
        highlight={{ rect: { x: 100, y: 80, width: 140, height: 40 }, text: "Нажмите «Сохранить как»", step: 2, total: 4 }}
      />,
    );
    expect(html).toContain("m27-highlight-layer is-contained is-reduced");
    expect(html).toContain("Шаг 2 из 4");
    expect(html).toContain("Нажмите «Сохранить как»");
  });

  it("renders GuidePanel with goal, step badge, Back, and Stop", () => {
    setLanguage("en");
    const html = renderToStaticMarkup(
      <GuidePanel
        guide={{ goal: "Save as PDF", step: 2, total: 3, text: "Click Export", active: true }}
      />,
    );
    expect(html).toContain("Save as PDF");
    expect(html).toContain("Step 2 of 3");
    expect(html).toContain("Click Export");
    expect(html).toContain("Back");
    expect(html).toContain("Stop");
  });
});
