// (No import from ./colors: colors.ts uses this file.)
function parseHex(color: string): [number, number, number] | null {
  const m = /^#([0-9a-fA-F]{6})$/.exec(color);
  if (!m || !m[1]) return null;
  return [parseInt(m[1].slice(0, 2), 16), parseInt(m[1].slice(2, 4), 16), parseInt(m[1].slice(4, 6), 16)];
}

const channel = (v: number): number => {
  const s = v / 255;
  return s <= 0.03928 ? s / 12.92 : ((s + 0.055) / 1.055) ** 2.4;
};

/** WCAG relative luminance of `#rrggbb` (0 for anything else). */
export function relativeLuminance(color: string): number {
  const rgb = parseHex(color);
  if (!rgb) return 0;
  return 0.2126 * channel(rgb[0]) + 0.7152 * channel(rgb[1]) + 0.0722 * channel(rgb[2]);
}

/** WCAG contrast ratio of two `#rrggbb` colours, 1..21. */
export function contrast(a: string, b: string): number {
  const [hi, lo] = [relativeLuminance(a), relativeLuminance(b)].sort((x, y) => y - x) as [number, number];
  return (hi + 0.05) / (lo + 0.05);
}

/** `#rrggbb` of an `r,g,b` string such as `"63,118,237"`. */
export function hexOfRgb(rgb: string): string {
  return `#${rgb
    .split(",")
    .map((v) => Number(v).toString(16).padStart(2, "0"))
    .join("")}`;
}

const LIGHT_LABEL = "#ffffff";
const DARK_LABEL = "#0b0b0e";
/** WCAG AA for text. */
export const MIN_CONTRAST = 4.5;

/**
 * The label colour for a key face that is a gradient from `top` to `bottom`: white or near-black, whichever reads
 * better on the worse of the two ends. Light keys (yellow, cyan) get dark labels instead of an unreadable white.
 */
export function readableOn(top: string, bottom: string): string {
  const worst = (label: string) => Math.min(contrast(label, top), contrast(label, bottom));
  return worst(LIGHT_LABEL) >= worst(DARK_LABEL) ? LIGHT_LABEL : DARK_LABEL;
}

/** `hex` mixed with black by `t` (0..1). */
function towardBlack(hex: string, t: number): string {
  const rgb = parseHex(hex);
  if (!rgb) return hex;
  return `#${rgb.map((c) => Math.round(c * (1 - t)).toString(16).padStart(2, "0")).join("")}`;
}

/**
 * The face of a coloured key that is readable: a label colour and the gradient ends. A white or near-black label is
 * taken when one of them reaches 4.5:1 on both ends. For the mid tones where neither does, the face is darkened just
 * enough for a white label to pass, so the key keeps its hue and the label stays readable.
 */
export function readableFace(top: string, bottom: string): { top: string; bottom: string; label: string } {
  const worst = (label: string, a: string, b: string) => Math.min(contrast(label, a), contrast(label, b));
  if (worst(LIGHT_LABEL, top, bottom) >= MIN_CONTRAST) return { top, bottom, label: LIGHT_LABEL };
  if (worst(DARK_LABEL, top, bottom) >= MIN_CONTRAST) return { top, bottom, label: DARK_LABEL };
  for (let t = 0.02; t <= 1; t += 0.02) {
    const a = towardBlack(top, t);
    const b = towardBlack(bottom, t);
    if (worst(LIGHT_LABEL, a, b) >= MIN_CONTRAST) return { top: a, bottom: b, label: LIGHT_LABEL };
  }
  return { top: "#000000", bottom: "#000000", label: LIGHT_LABEL };
}
