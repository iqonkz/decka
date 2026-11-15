import { hexOfRgb, readableFace } from "./contrast";

/** Accent colour when the layout names none (DATA_MODEL.md 3.1). */
export const DEFAULT_ACCENT = "#7c5cff";

/** `#rgb` / `#rrggbb` -> [r, g, b]; anything else -> null. */
export function parseHex(color: string): [number, number, number] | null {
  const m = /^#([0-9a-fA-F]{6})$/.exec(color) ?? /^#([0-9a-fA-F]{3})$/.exec(color);
  if (!m || !m[1]) return null;
  const h = m[1].length === 3 ? [...m[1]].map((c) => c + c).join("") : m[1];
  return [parseInt(h.slice(0, 2), 16), parseInt(h.slice(2, 4), 16), parseInt(h.slice(4, 6), 16)];
}

const mix = (c: number, target: number, amount: number): number => Math.round(c + (target - c) * amount);

/** `rgb` mixed with white (`amount` > 0) or black (`amount` < 0) in sRGB, as `r,g,b`. */
export function shade([r, g, b]: [number, number, number], amount: number): string {
  const target = amount >= 0 ? 255 : 0;
  const a = Math.abs(amount);
  return `${mix(r, target, a)},${mix(g, target, a)},${mix(b, target, a)}`;
}

export const luminance = ([r, g, b]: [number, number, number]): number => 0.2126 * r + 0.7152 * g + 0.0722 * b;

/**
 * CSS variables of a coloured key, so that every theme can build its own look from one colour:
 * `--c`, `--c-top` (12% lighter), `--c-bottom` (18% darker), `--c-a16/a30/a50` (the colour with that opacity)
 * and `--c-ink` (the colour itself, or a light grey when it is too dark to read on black).
 * No valid colour: no variables.
 */
export function colorVars(color: string | undefined): Record<string, string> {
  const rgb = color ? parseHex(color) : null;
  if (!rgb || !color) return {};
  const [r, g, b] = rgb;
  // The gradient of Studio/Glass; where the colour is a mid tone the face is darkened until its label is readable.
  const face = readableFace(hexOfRgb(shade(rgb, 0.12)), hexOfRgb(shade(rgb, -0.18)));
  return {
    "--c": color,
    "--c-on": face.label,
    "--c-on-shadow": face.label === "#ffffff" ? "0 1px 2px rgba(0,0,0,0.9)" : "none",
    "--c-top": face.top,
    "--c-bottom": face.bottom,
    "--c-a16": `rgba(${r},${g},${b},0.16)`,
    "--c-a30": `rgba(${r},${g},${b},0.30)`,
    "--c-a32": `rgba(${r},${g},${b},0.32)`,
    "--c-a50": `rgba(${r},${g},${b},0.5)`,
    "--c-ink": luminance(rgb) < 64 ? "#e5e5e5" : color,
  };
}
