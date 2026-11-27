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

  test("light colours keep their face and get the dark label (white, yellow)", () => {
    for (const color of ["#ffffff", "#facc15"]) {
      const vars = colorVars(color);
      expect(vars["--c-on"], color).toBe("#0b0b0f");
      expect(vars["--c-on-shadow"], color).toBe("none");
      expect(vars["--c-top"], color).toBe(hexOfRgb(shade(parseHex(color)!, 0.12)));
      expect(vars["--c-bottom"], color).toBe(hexOfRgb(shade(parseHex(color)!, -0.18)));
      expect(contrast("#0b0b0f", color), color).toBeGreaterThanOrEqual(7);
    }
  });
  test("#7c5cff is darkened and gets the white label; every result reads >= 4.5", () => {
    const vars = colorVars("#7c5cff");
    expect(vars["--c-on"]).toBe("#ffffff");
    for (const end of ["--c-top", "--c-bottom"]) {
      const v = vars[end]!;
      expect(contrast("#ffffff", v.startsWith("#") ? v : hexOfRgb(v.slice(4, -1)))).toBeGreaterThanOrEqual(MIN);
    }
    for (const color of ["#ffffff", "#facc15", "#ef4444", "#16a34a"]) {
      const v = colorVars(color);
      const hex = (s: string) => (s.startsWith("#") ? s : hexOfRgb(s.slice(4, -1)));
      expect(contrast(v["--c-on"]!, hex(v["--c-top"]!)), color).toBeGreaterThanOrEqual(MIN);
      expect(contrast(v["--c-on"]!, hex(v["--c-bottom"]!)), color).toBeGreaterThanOrEqual(MIN);
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

describe("M31 settings & island UI tokens contrast (WCAG AA >= 4.5:1)", () => {
  test("light and dark settings theme tokens satisfy >= 4.5:1 on surfaces", () => {
    const light = { bg: "#fafafa", panel: "#ffffff", hover: "#ececf0", text: "#1a1a1f", muted: "#5f6672", ok: "#166534", warn: "#9a4306", danger: "#b91c1c", accent: "#2563eb" };
    const dark = { bg: "#141417", panel: "#1b1b1f", hover: "#26262c", text: "#e8e8ee", muted: "#9ca3af", ok: "#4ade80", warn: "#fbbf24", danger: "#f87171", accent: "#60a5fa" };
    for (const [name, tok] of [["light", light], ["dark", dark]] as const) {
      for (const fg of ["text", "muted", "ok", "warn", "danger", "accent"] as const) {
        expect(contrast(tok[fg], tok.panel), `${name} ${fg} on panel`).toBeGreaterThanOrEqual(MIN);
        expect(contrast(tok[fg], tok.hover), `${name} ${fg} on hover`).toBeGreaterThanOrEqual(MIN);
      }
    }
    // Primary button white text on light default accent (#2563eb) and darkened Decka purple (#694ee0).
    expect(contrast("#ffffff", "#2563eb")).toBeGreaterThanOrEqual(MIN);
    expect(contrast("#ffffff", "#694ee0")).toBeGreaterThanOrEqual(MIN);
  });
});
