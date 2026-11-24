export interface RGB {
  r: number;
  g: number;
  b: number;
}

export interface HSV {
  h: number; // 0..360
  s: number; // 0..1
  v: number; // 0..1
}

export function clamp(val: number, min: number, max: number): number {
  return Math.max(min, Math.min(max, val));
}

/** Parses #rgb, #rgba, #rrggbb, #rrggbbaa into RGB (0..255). Returns null on invalid. */
export function hexToRgb(hex: string): RGB | null {
  const clean = hex.trim().replace(/^#/, "");
  let r = 0;
  let g = 0;
  let b = 0;

  if (clean.length === 3 || clean.length === 4) {
    const c0 = clean[0] ?? "";
    const c1 = clean[1] ?? "";
    const c2 = clean[2] ?? "";
    r = parseInt(c0 + c0, 16);
    g = parseInt(c1 + c1, 16);
    b = parseInt(c2 + c2, 16);
  } else if (clean.length === 6 || clean.length === 8) {
    r = parseInt(clean.slice(0, 2), 16);
    g = parseInt(clean.slice(2, 4), 16);
    b = parseInt(clean.slice(4, 6), 16);
  } else {
    return null;
  }

  if (Number.isNaN(r) || Number.isNaN(g) || Number.isNaN(b)) return null;
  return { r: clamp(r, 0, 255), g: clamp(g, 0, 255), b: clamp(b, 0, 255) };
}

/** Converts r, g, b (0..255) into standard 6-digit hex "#rrggbb". */
export function rgbToHex(r: number, g: number, b: number): string {
  const toHex = (n: number) => {
    const clamped = Math.round(clamp(n, 0, 255));
    return clamped.toString(16).padStart(2, "0");
  };
  return `#${toHex(r)}${toHex(g)}${toHex(b)}`;
}

/** Converts RGB (0..255) into HSV (h: 0..360, s: 0..1, v: 0..1). */
export function rgbToHsv(r: number, g: number, b: number): HSV {
  const rn = clamp(r, 0, 255) / 255;
  const gn = clamp(g, 0, 255) / 255;
  const bn = clamp(b, 0, 255) / 255;

  const max = Math.max(rn, gn, bn);
  const min = Math.min(rn, gn, bn);
  const delta = max - min;

  let h = 0;
  if (delta > 0) {
    if (max === rn) {
      h = 60 * (((gn - bn) / delta) % 6);
    } else if (max === gn) {
      h = 60 * ((bn - rn) / delta + 2);
    } else {
      h = 60 * ((rn - gn) / delta + 4);
    }
  }
  if (h < 0) h += 360;

  const s = max === 0 ? 0 : delta / max;
  const v = max;

  return {
    h,
    s: clamp(s, 0, 1),
    v: clamp(v, 0, 1),
  };
}

/** Converts HSV (h: 0..360, s: 0..1, v: 0..1) into RGB (0..255). */
export function hsvToRgb(h: number, s: number, v: number): RGB {
  const hc = (h % 360 + 360) % 360;
  const sc = clamp(s, 0, 1);
  const vc = clamp(v, 0, 1);

  const c = vc * sc;
  const x = c * (1 - Math.abs(((hc / 60) % 2) - 1));
  const m = vc - c;

  let r1 = 0;
  let g1 = 0;
  let b1 = 0;

  if (hc >= 0 && hc < 60) {
    r1 = c;
    g1 = x;
  } else if (hc >= 60 && hc < 120) {
    r1 = x;
    g1 = c;
  } else if (hc >= 120 && hc < 180) {
    g1 = c;
    b1 = x;
  } else if (hc >= 180 && hc < 240) {
    g1 = x;
    b1 = c;
  } else if (hc >= 240 && hc < 300) {
    r1 = x;
    b1 = c;
  } else {
    r1 = c;
    b1 = x;
  }

  return {
    r: Math.round((r1 + m) * 255),
    g: Math.round((g1 + m) * 255),
    b: Math.round((b1 + m) * 255),
  };
}

/** Converts hex string to HSV. Returns fallback for #7c5cff on invalid. */
export function hexToHsv(hex: string): HSV {
  const rgb = hexToRgb(hex) ?? { r: 124, g: 92, b: 255 };
  return rgbToHsv(rgb.r, rgb.g, rgb.b);
}

/** Converts HSV to 6-digit hex string. */
export function hsvToHex(h: number, s: number, v: number): string {
  const rgb = hsvToRgb(h, s, v);
  return rgbToHex(rgb.r, rgb.g, rgb.b);
}

/** Ensures string is valid 6-character hex "#rrggbb" or returns default. */
export function normalizeHex(hex: string, fallback = "#7c5cff"): string {
  const rgb = hexToRgb(hex);
  if (!rgb) return fallback;
  return rgbToHex(rgb.r, rgb.g, rgb.b);
}
