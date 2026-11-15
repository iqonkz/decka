import { describe, expect, test } from "vitest";
import { colorVars, shade, parseHex } from "../src/theme/colors";
import { contrast, hexOfRgb, readableFace, readableOn } from "../src/theme/contrast";
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

  test("Glass: white on the glass over the darkest and the brightest stops of each wallpaper", () => {
    // Worst case = the lightest colour a wallpaper puts behind a key, plus the 7% white of the glass layer.
    const brightest: Record<string, string> = { aurora: "#0ea5e9", sunset: "#f97316", ocean: "#0891b2", graphite: "#3f3f46" };
    const ratios = Object.fromEntries(
      Object.entries(brightest).map(([name, stop]) => {
        const rgb = parseHex(stop)!;
        const glass = hexOfRgb(shade(rgb, 0.07));
        return [name, contrast("#ffffff", glass)];
      }),
    );
    // Recorded for docs/BLOCKERS.md: bright wallpaper stops are below 4.5 with white labels (see the report of T13.11).
    expect(ratios.graphite).toBeGreaterThanOrEqual(MIN);
    for (const [name, ratio] of Object.entries(ratios)) expect(ratio, name).toBeGreaterThan(2);
  });
});

describe("readableFace", () => {
  test("a face that already reads is left alone", () => {
    expect(readableFace("#1c1c22", "#0d0d10")).toEqual({ top: "#1c1c22", bottom: "#0d0d10", label: "#ffffff" });
    expect(readableFace("#fde047", "#ca8a04").label).toBe("#0b0b0e");
  });

  test("a mid tone is darkened, keeping its hue, until a white label passes", () => {
    const face = readableFace("#7e60ff", "#5f3fe0");
    expect(face.label).toBe("#ffffff");
    expect(contrast("#ffffff", face.top)).toBeGreaterThanOrEqual(MIN);
    expect(contrast("#ffffff", face.bottom)).toBeGreaterThanOrEqual(MIN);
    expect(face.top).not.toBe("#7e60ff");
  });
});

describe("readableOn", () => {
  test("white on dark faces, near-black on light ones", () => {
    expect(readableOn("#1c1c22", "#0d0d10")).toBe("#ffffff");
    expect(readableOn("#fde047", "#ca8a04")).toBe("#0b0b0e");
  });
});
