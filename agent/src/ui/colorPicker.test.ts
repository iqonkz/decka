import { describe, it, expect } from "vitest";
import {
  hexToRgb,
  rgbToHex,
  rgbToHsv,
  hsvToRgb,
  hexToHsv,
  hsvToHex,
  normalizeHex,
} from "./colorUtils";

describe("ColorPicker utils (HEX <-> HSV/RGB)", () => {
  it("converts 6-digit HEX to RGB and back", () => {
    const red = hexToRgb("#ff0000");
    expect(red).toEqual({ r: 255, g: 0, b: 0 });
    expect(rgbToHex(255, 0, 0)).toBe("#ff0000");

    const blue = hexToRgb("#0000ff");
    expect(blue).toEqual({ r: 0, g: 0, b: 255 });
    expect(rgbToHex(0, 0, 255)).toBe("#0000ff");

    const violet = hexToRgb("#7c5cff");
    expect(violet).toEqual({ r: 124, g: 92, b: 255 });
    expect(rgbToHex(124, 92, 255)).toBe("#7c5cff");
  });

  it("handles 3-digit shorthand HEX", () => {
    expect(hexToRgb("#fff")).toEqual({ r: 255, g: 255, b: 255 });
    expect(hexToRgb("#000")).toEqual({ r: 0, g: 0, b: 0 });
    expect(hexToRgb("#f0a")).toEqual({ r: 255, g: 0, b: 170 });
  });

  it("returns null for invalid HEX", () => {
    expect(hexToRgb("invalid")).toBeNull();
    expect(hexToRgb("#12")).toBeNull();
    expect(hexToRgb("#gggggg")).toBeNull();
  });

  it("converts RGB to HSV and back for primary colors", () => {
    // Red: H=0, S=1, V=1
    const hsvRed = rgbToHsv(255, 0, 0);
    expect(hsvRed.h).toBe(0);
    expect(hsvRed.s).toBe(1);
    expect(hsvRed.v).toBe(1);
    expect(hsvToRgb(0, 1, 1)).toEqual({ r: 255, g: 0, b: 0 });

    // Green: H=120, S=1, V=1
    const hsvGreen = rgbToHsv(0, 255, 0);
    expect(hsvGreen.h).toBe(120);
    expect(hsvGreen.s).toBe(1);
    expect(hsvGreen.v).toBe(1);
    expect(hsvToRgb(120, 1, 1)).toEqual({ r: 0, g: 255, b: 0 });

    // Blue: H=240, S=1, V=1
    const hsvBlue = rgbToHsv(0, 0, 255);
    expect(hsvBlue.h).toBe(240);
    expect(hsvBlue.s).toBe(1);
    expect(hsvBlue.v).toBe(1);
    expect(hsvToRgb(240, 1, 1)).toEqual({ r: 0, g: 0, b: 255 });

    // Black: V=0
    const hsvBlack = rgbToHsv(0, 0, 0);
    expect(hsvBlack.v).toBe(0);
    expect(hsvToRgb(0, 0, 0)).toEqual({ r: 0, g: 0, b: 0 });

    // White: S=0, V=1
    const hsvWhite = rgbToHsv(255, 255, 255);
    expect(hsvWhite.s).toBe(0);
    expect(hsvWhite.v).toBe(1);
    expect(hsvToRgb(0, 0, 1)).toEqual({ r: 255, g: 255, b: 255 });
  });

  it("round-trips HEX -> HSV -> HEX accurately", () => {
    const testColors = [
      "#7c5cff", // Decka violet
      "#3b82f6", // Blue
      "#06b6d4", // Cyan
      "#16a34a", // Green
      "#eab308", // Yellow
      "#f97316", // Orange
      "#ef4444", // Red
      "#ec4899", // Pink
    ];

    for (const color of testColors) {
      const hsv = hexToHsv(color);
      const roundTrip = hsvToHex(hsv.h, hsv.s, hsv.v);
      expect(roundTrip.toLowerCase()).toBe(color.toLowerCase());
    }
  });

  it("normalizes hex strings correctly", () => {
    expect(normalizeHex("#7C5CFF")).toBe("#7c5cff");
    expect(normalizeHex("#fff")).toBe("#ffffff");
    expect(normalizeHex("not-a-color", "#112233")).toBe("#112233");
  });
});
