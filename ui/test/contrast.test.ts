import { describe, expect, test } from "vitest";
import { colorVars, shade, parseHex } from "../src/theme/colors";
import { contrast, hexOfRgb, readableFace } from "../src/theme/contrast";
import { materialRoles } from "../src/theme/material";

/** The 8 accents of the Appearance tab (DESIGN.md 4.4, T13.10). */
const ACCENTS = ["#7c5cff", "#3b82f6", "#06b6d4", "#16a34a", "#eab308", "#f97316", "#ef4444", "#ec4899"];
const MIN = 4.5;

describe("contrast", () => {
  test("black on white is 21, a colour on itself is 1, order does not matter", () => {
    expect(contrast("#000000", "#ffffff")).toBeCloseTo(21, 5);
    expect(contrast("#7c5cff", "#7c5cff")).toBeCloseTo(1, 5);
    expect(contrast("#123456", "#abcdef")).toBeCloseTo(contrast("#abcdef", "#123456"), 10);
  });
});

describe("labels are readable in every theme (WCAG AA, >= 4.5:1)", () => {
  test("AMOLED: both label colours on the black screen", () => {
    expect(contrast("#9ca3af", "#000000")).toBeGreaterThanOrEqual(MIN); // plain keys
    expect(contrast("#e5e5e5", "#000000")).toBeGreaterThanOrEqual(MIN); // coloured keys and icons
  });

  test("Studio: white on both ends of the plain face", () => {
    for (const end of ["#1c1c22", "#0d0d10"]) expect(contrast("#ffffff", end)).toBeGreaterThanOrEqual(MIN);
  });

  test("Studio: a coloured key reads whatever its colour (all accents and the shipped key colours)", () => {
    const colours = [...ACCENTS, "#b91c1c", "#b45309", "#2563eb", "#1e1e24", "#ffffff", "#000000", "#fde047", "#22d3ee"];
    for (const color of colours) {
      const vars = colorVars(color);
      const label = vars["--c-on"]!;
      // The face the key really gets: the CSS variables are `rgb(r,g,b)` or `#rrggbb`.
      const hex = (v: string) => (v.startsWith("#") ? v : hexOfRgb(v.slice(4, -1)));
      expect(contrast(label, hex(vars["--c-top"]!)), `${color} top`).toBeGreaterThanOrEqual(MIN);
      expect(contrast(label, hex(vars["--c-bottom"]!)), `${color} bottom`).toBeGreaterThanOrEqual(MIN);
    }
  });

  test("Material: the label colours of every accent, plain and coloured keys", () => {
    for (const accent of ACCENTS) {
      const r = materialRoles(accent);
      expect(contrast(r.onSurfaceVariant, r.surfaceContainerHigh), `${accent} label`).toBeGreaterThanOrEqual(MIN);
      expect(contrast(r.onSurface, r.surfaceContainerHigh), `${accent} icon`).toBeGreaterThanOrEqual(MIN);
      expect(contrast(r.onSurfaceVariant, r.surfaceContainer), `${accent} panel`).toBeGreaterThanOrEqual(MIN);
      expect(contrast(r.onPrimaryContainer, r.primaryContainer), `${accent} coloured key`).toBeGreaterThanOrEqual(MIN);
    }
  });

  // Every colour stop of every wallpaper except `album` (DESIGN.md 1.5), under the 0.35 black dimming layer (D-1) and
  // the 7% white of the glass layer. This is the worst case: a key sits over a stop at full strength.
  const STOPS: Record<string, string[]> = {
    aurora: ["#6d28d9", "#0ea5e9", "#10b981"],
    sunset: ["#f97316", "#db2777", "#7c3aed"],
    ocean: ["#1d4ed8", "#0891b2", "#0f766e"],
    graphite: ["#3f3f46", "#27272a"],
  };
  const MUTED_GLASS = "#f9fafb"; // `--muted` in glass.css
  const glassOver = (stop: string): string => {
    const dimmed = shade(parseHex(stop)!, -0.35);
    return hexOfRgb(shade(dimmed.split(",").map(Number) as [number, number, number], 0.07));
  };

  test("Glass: white labels on the dimmed glass over every wallpaper stop, strictly >= 4.5", () => {
    for (const [name, stops] of Object.entries(STOPS))
      for (const stop of stops) expect(contrast("#ffffff", glassOver(stop)), `${name} ${stop}`).toBeGreaterThanOrEqual(MIN);
  });

  test("Glass: secondary text (--muted) on the glass over every wallpaper stop", () => {
    for (const [name, stops] of Object.entries(STOPS))
      for (const stop of stops) expect(contrast(MUTED_GLASS, glassOver(stop)), `${name} ${stop}`).toBeGreaterThanOrEqual(MIN);
  });

  test("secondary text on the other themes: #9ca3af on the panel", () => {
    expect(contrast("#9ca3af", "#000000")).toBeGreaterThanOrEqual(MIN); // AMOLED
    for (const end of ["#1c1c22", "#0d0d10"]) expect(contrast("#9ca3af", end), `Studio ${end}`).toBeGreaterThanOrEqual(MIN);
  });
});

describe("readableFace", () => {
  test("a face that already reads is left alone", () => {
    expect(readableFace("#1c1c22", "#0d0d10")).toEqual({ top: "#1c1c22", bottom: "#0d0d10", label: "#ffffff" });
  });

  test("the label is always white; light colours are darkened, never given a dark label", () => {
    for (const [top, bottom] of [["#fde047", "#ca8a04"], ["#ffffff", "#e5e5e5"], ["#22d3ee", "#0891b2"]] as const) {
      const face = readableFace(top, bottom);
      expect(face.label).toBe("#ffffff");
      expect(contrast("#ffffff", face.top)).toBeGreaterThanOrEqual(MIN);
      expect(contrast("#ffffff", face.bottom)).toBeGreaterThanOrEqual(MIN);
    }
  });

  test("a mid tone is darkened, keeping its hue, until a white label passes", () => {
    const face = readableFace("#7e60ff", "#5f3fe0");
    expect(face.label).toBe("#ffffff");
    expect(contrast("#ffffff", face.top)).toBeGreaterThanOrEqual(MIN);
    expect(contrast("#ffffff", face.bottom)).toBeGreaterThanOrEqual(MIN);
    expect(face.top).not.toBe("#7e60ff");
  });
});
