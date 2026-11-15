import { describe, expect, test } from "vitest";
import { contrast, hexOfRgb } from "../src/theme/contrast";
import { faceLayout, isTileCorners, labelShown, LABEL_GRADIENT_ALPHA, resolveFace } from "../src/faceLayout";

const PNG = "data:image/png;base64,AA";
const SVG = "data:image/svg+xml;base64,AA";

describe("faceLayout (DESIGN.md 2.1, one test per row of the table)", () => {
  test("an opaque square picture (tile) covers the whole key", () => {
    expect(faceLayout("img:0123456789abcdef", true, 100, false, PNG)).toEqual({ mode: "cover", iconPx: 100, overlayLabel: false });
  });
  test("a transparent program picture is contained at 0.84 of the key", () => {
    expect(faceLayout("img:0123456789abcdef", false, 100, true, PNG)).toEqual({ mode: "contain", iconPx: 84, overlayLabel: true });
    expect(faceLayout("img:0123456789abcdef", false, 240, true)).toMatchObject({ mode: "contain", iconPx: 202 }); // url not known yet: a picture
  });
  test("lucide, emoji and an SVG are centred at 0.68 of the key", () => {
    expect(faceLayout("lucide:play", false, 100, true)).toEqual({ mode: "center", iconPx: 68, overlayLabel: true });
    expect(faceLayout("emoji:🎵", false, 100, false)).toEqual({ mode: "center", iconPx: 68, overlayLabel: false });
    expect(faceLayout("img:0123456789abcdef", false, 100, true, SVG)).toEqual({ mode: "center", iconPx: 68, overlayLabel: true });
    // an SVG is never a tile, even when asked
    expect(faceLayout("img:0123456789abcdef", true, 100, true, SVG).mode).toBe("center");
  });
  test("no icon, no layout", () => {
    expect(faceLayout(undefined, false, 100, true)).toEqual({ mode: "none", iconPx: 0, overlayLabel: false });
  });
});

describe("tile detection", () => {
  test("all four corners must be nearly opaque (alpha >= 250)", () => {
    expect(isTileCorners([255, 255, 255, 255])).toBe(true);
    expect(isTileCorners([250, 251, 252, 253])).toBe(true);
    expect(isTileCorners([255, 255, 255, 249])).toBe(false);
    expect(isTileCorners([0, 0, 0, 0])).toBe(false);
  });
});

describe("which face and whether the label shows", () => {
  test("own face, else the display's, else standard", () => {
    expect(resolveFace("full", "standard")).toBe("full");
    expect(resolveFace(undefined, "full")).toBe("full");
    expect(resolveFace(undefined, undefined)).toBe("standard");
    expect(resolveFace("standard", "full")).toBe("standard");
  });
  test("a label shows when it is not empty and (showLabel ?? keyLabels ?? true)", () => {
    expect(labelShown("Spotify", undefined, undefined)).toBe(true);
    expect(labelShown("Spotify", undefined, false)).toBe(false);
    expect(labelShown("Spotify", true, false)).toBe(true);
    expect(labelShown("Spotify", false, true)).toBe(false);
    expect(labelShown("", undefined, undefined)).toBe(false);
    expect(labelShown(undefined, true, true)).toBe(false);
  });
});

describe("the white label over the dark gradient", () => {
  test("even on a pure white picture the 0.72 layer keeps white text at >= 4.5:1", () => {
    // The lightest colour under the gradient is white; the layer makes it 255 * (1 - 0.72).
    const under = Math.round(255 * (1 - LABEL_GRADIENT_ALPHA));
    expect(contrast("#ffffff", hexOfRgb(`${under},${under},${under}`))).toBeGreaterThanOrEqual(4.5);
  });
});