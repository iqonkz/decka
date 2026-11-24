/**
 * Cover art color sampling & darkening for the Media widget (DESIGN.md 4.16).
 * Samples a 16x16 canvas of the artwork, averages opaque pixels, and darkens to HSL lightness <= 30% so white text reads clearly.
 */

export type Rgb = readonly [number, number, number];

export const COVER_MAX_LIGHTNESS = 0.30;
const FALLBACK_RGB: Rgb = [32, 34, 46];
const GRID_SAMPLE_SIZE = 16;

const clamp01 = (n: number): number => Math.min(1, Math.max(0, n));
const toHexByte = (n: number): string => Math.round(Math.min(255, Math.max(0, n))).toString(16).padStart(2, "0");

export function rgbToHex([r, g, b]: Rgb): string {
  return `#${toHexByte(r)}${toHexByte(g)}${toHexByte(b)}`;
}

/** Converts sRGB 0..255 to [h, s, l] where h in [0, 360), s and l in [0, 1]. */
export function rgbToHsl([r, g, b]: Rgb): [number, number, number] {
  const rn = r / 255;
  const gn = g / 255;
  const bn = b / 255;
  const max = Math.max(rn, gn, bn);
  const min = Math.min(rn, gn, bn);
  const l = (max + min) / 2;
  if (max === min) return [0, 0, l];
  const d = max - min;
  const s = l > 0.5 ? d / (2 - max - min) : d / (max + min);
  let h = 0;
  if (max === rn) h = (gn - bn) / d + (gn < bn ? 6 : 0);
  else if (max === gn) h = (bn - rn) / d + 2;
  else h = (rn - gn) / d + 4;
  return [h * 60, s, l];
}

/** Converts HSL (h in 0..360, s/l in 0..1) back to sRGB 0..255. */
export function hslToRgb(h: number, s: number, l: number): Rgb {
  if (s <= 0) {
    const v = Math.round(clamp01(l) * 255);
    return [v, v, v];
  }
  const hue = (((h % 360) + 360) % 360) / 360;
  const q = l < 0.5 ? l * (1 + s) : l + s - l * s;
  const p = 2 * l - q;
  const channel = (t: number) => {
    let x = t;
    if (x < 0) x += 1;
    if (x > 1) x -= 1;
    if (x < 1 / 6) return p + (q - p) * 6 * x;
    if (x < 1 / 2) return q;
    if (x < 2 / 3) return p + (q - p) * (2 / 3 - x) * 6;
    return p;
  };
  return [
    Math.round(clamp01(channel(hue + 1 / 3)) * 255),
    Math.round(clamp01(channel(hue)) * 255),
    Math.round(clamp01(channel(hue - 1 / 3)) * 255),
  ];
}

/** Averages non-transparent pixels from an RGBA pixel buffer (e.g. 16x16 canvas ImageData). */
export function averageRgbFromPixels(data: ArrayLike<number>): Rgb {
  let rSum = 0;
  let gSum = 0;
  let bSum = 0;
  let count = 0;
  for (let i = 0; i + 3 < data.length; i += 4) {
    const alpha = data[i + 3] ?? 255;
    if (alpha < 16) continue;
    rSum += data[i] ?? 0;
    gSum += data[i + 1] ?? 0;
    bSum += data[i + 2] ?? 0;
    count++;
  }
  if (count === 0) return FALLBACK_RGB;
  return [Math.round(rSum / count), Math.round(gSum / count), Math.round(bSum / count)];
}

/** Darkens an RGB colour so its HSL lightness is at most `maxLightness` (default 0.30 = 30%). */
export function darkenCoverColor(
  rgb: Rgb,
  maxLightness = COVER_MAX_LIGHTNESS,
): { rgb: Rgb; hex: string; lightness: number } {
  const [h, s, l] = rgbToHsl(rgb);
  const clampedL = Math.min(l, clamp01(maxLightness));
  const outRgb = hslToRgb(h, s, clampedL);
  const [, , actualL] = rgbToHsl(outRgb);
  return { rgb: outRgb, hex: rgbToHex(outRgb), lightness: actualL };
}

/** Builds a rich dark gradient from the sampled artwork color with L <= 30% for readable white text. */
export function coverGradient(rgb: Rgb): string {
  const primary = darkenCoverColor(rgb, COVER_MAX_LIGHTNESS);
  const deep = darkenCoverColor(rgb, Math.min(0.14, primary.lightness * 0.55));
  return `linear-gradient(145deg, ${primary.hex} 0%, ${deep.hex} 100%)`;
}

/** Samples an image URL in a 16x16 offscreen canvas; returns null if unavailable or unreadable. */
export function sampleImageColor(src: string): Promise<Rgb | null> {
  if (typeof document === "undefined" || !src) return Promise.resolve(null);
  return new Promise((resolve) => {
    const img = new Image();
    img.crossOrigin = "anonymous";
    img.onload = () => {
      try {
        const canvas = document.createElement("canvas");
        canvas.width = GRID_SAMPLE_SIZE;
        canvas.height = GRID_SAMPLE_SIZE;
        const ctx = canvas.getContext("2d", { willReadFrequently: true });
        if (!ctx) return resolve(null);
        ctx.drawImage(img, 0, 0, GRID_SAMPLE_SIZE, GRID_SAMPLE_SIZE);
        const pixels = ctx.getImageData(0, 0, GRID_SAMPLE_SIZE, GRID_SAMPLE_SIZE).data;
        resolve(averageRgbFromPixels(pixels));
      } catch {
        resolve(null);
      }
    };
    img.onerror = () => resolve(null);
    img.src = src;
  });
}
