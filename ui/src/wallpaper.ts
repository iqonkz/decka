import type { WallpaperId } from "@decka/shared";

/**
 * The wallpaper of the Glass theme, drawn once into a canvas of a quarter of the screen with the blur already applied
 * (DESIGN.md 1.5, "pre-blurred background") and, in `reduced`, the "frosted" copy for the keys (DESIGN.md 1.4).
 * The pure parts (sizes, noise, positions, gradient data) have no DOM and are unit-tested.
 */

/** One `radial-gradient(rx ry at cx cy, colour 0%, transparent end)` of DESIGN.md 1.5, all numbers as fractions. */
export interface GradientLayer {
  cx: number;
  cy: number;
  rx: number;
  ry: number;
  color: string;
  /** Where the colour has faded to transparent (0..1 of the radii). */
  end: number;
}

export type GradientWallpaper = Exclude<WallpaperId, "album">;

/** Colour under the gradients (the CSS `background-color` of the layer). */
export const WALLPAPER_BASE = "#05060a";
/** The solid dimming layer over the gradients (D-1); `album` dims itself with `brightness(0.55)`. */
export const WALLPAPER_DIM = 0.35;

/** The definitions of DESIGN.md 1.5, in CSS order (the first layer is the topmost). */
export const WALLPAPER_LAYERS: Record<GradientWallpaper, GradientLayer[]> = {
  aurora: [
    { cx: 0.15, cy: 0.2, rx: 0.6, ry: 0.8, color: "#6d28d9", end: 0.6 },
    { cx: 0.85, cy: 0.3, rx: 0.5, ry: 0.7, color: "#0ea5e9", end: 0.6 },
    { cx: 0.5, cy: 1, rx: 0.7, ry: 0.6, color: "#10b981", end: 0.65 },
  ],
  sunset: [
    { cx: 0.2, cy: 0.3, rx: 0.6, ry: 0.8, color: "#f97316", end: 0.6 },
    { cx: 0.8, cy: 0.2, rx: 0.6, ry: 0.8, color: "#db2777", end: 0.6 },
    { cx: 0.5, cy: 1, rx: 0.8, ry: 0.6, color: "#7c3aed", end: 0.65 },
  ],
  ocean: [
    { cx: 0.2, cy: 0.2, rx: 0.6, ry: 0.8, color: "#1d4ed8", end: 0.6 },
    { cx: 0.8, cy: 0.6, rx: 0.6, ry: 0.7, color: "#0891b2", end: 0.6 },
    { cx: 0.4, cy: 1, rx: 0.7, ry: 0.6, color: "#0f766e", end: 0.65 },
  ],
  graphite: [
    { cx: 0.3, cy: 0.2, rx: 0.7, ry: 0.9, color: "#3f3f46", end: 0.6 },
    { cx: 0.8, cy: 0.8, rx: 0.6, ry: 0.8, color: "#27272a", end: 0.6 },
  ],
};

/** The wallpaper layer is 116% of the screen (`inset: -8%`), so the screen is its central 1/1.16. */
export const LAYER_SCALE = 1.16;
/** Width of a canvas on an A57 (a quarter of the screen); the blur radii below are defined at this width. */
export const REFERENCE_WIDTH = 585;
/** Blur of the base, in canvas px at `REFERENCE_WIDTH` (≈ 48 px on the full screen). */
export const BASE_BLUR_PX = 12;
/** The extra blur of the frosted copy used by `reduced`. */
export const FROST_BLUR_PX = 6;
/** Largest and smallest side of the canvas. */
export const MAX_SIDE = 640;
export const MIN_SIDE = 64;
/** Noise against banding: every colour channel moves by at most this much. */
export const NOISE_AMPLITUDE = 3;

/** Size of the canvas for a wallpaper layer of `layerW` x `layerH` CSS px: a quarter of the screen in device pixels. */
export function bitmapSize(layerW: number, layerH: number, dpr: number): { width: number; height: number } {
  const quarter = (v: number) => (v * Math.max(1, dpr)) / LAYER_SCALE / 4;
  const longest = Math.max(quarter(layerW), quarter(layerH), 1);
  const k = longest > MAX_SIDE ? MAX_SIDE / longest : longest < MIN_SIDE ? MIN_SIDE / longest : 1;
  return { width: Math.max(1, Math.round(quarter(layerW) * k)), height: Math.max(1, Math.round(quarter(layerH) * k)) };
}

/** A blur radius defined at the A57 canvas, scaled to the canvas actually used. */
export function scaledBlur(px: number, canvasWidth: number): number {
  return Math.max(0.5, +((px * canvasWidth) / REFERENCE_WIDTH).toFixed(2));
}

/** Small seeded generator (mulberry32): the same wallpaper gives the same noise, so snapshots do not flicker. */
export function seeded(seed: number): () => number {
  let a = seed >>> 0;
  return () => {
    a = (a + 0x6d2b79f5) >>> 0;
    let t = a;
    t = Math.imul(t ^ (t >>> 15), t | 1);
    t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}

/** Moves every colour channel of RGBA pixels by a random whole number in [-amplitude, amplitude]; alpha stays. One pass. */
export function addNoise(rgba: Uint8ClampedArray, amplitude: number = NOISE_AMPLITUDE, seed: number = 1): void {
  const next = seeded(seed);
  const span = 2 * amplitude + 1;
  for (let i = 0; i < rgba.length; i += 4) {
    // One value for the three channels: a brightness change, not coloured speckles.
    const d = Math.floor(next() * span) - amplitude;
    rgba[i] = (rgba[i] as number) + d;
    rgba[i + 1] = (rgba[i + 1] as number) + d;
    rgba[i + 2] = (rgba[i + 2] as number) + d;
  }
}

export interface Rect {
  left: number;
  top: number;
  width: number;
  height: number;
}

/**
 * Where the frosted screen-sized picture lies inside a key so that it shows the piece of wallpaper under that key
 * (DESIGN.md 1.4, `reduced`): the picture is as large as the screen and is moved by minus the key's own position.
 */
export function frostPosition(keyRect: Rect, viewport: { width: number; height: number }): { x: number; y: number; width: number; height: number } {
  return { x: -Math.round(keyRect.left * 100) / 100, y: -Math.round(keyRect.top * 100) / 100, width: viewport.width, height: viewport.height };
}

/** The central part of the wallpaper canvas that the screen shows (the layer is 116% of the screen and does not drift in `reduced`). */
export function screenCrop(width: number, height: number): { sx: number; sy: number; sw: number; sh: number } {
  const sw = width / LAYER_SCALE;
  const sh = height / LAYER_SCALE;
  return { sx: (width - sw) / 2, sy: (height - sh) / 2, sw, sh };
}

// ---------------------------------------------------------------------------------------------------------------
// Canvas drawing (needs a DOM)

type Canvas = HTMLCanvasElement;

function newCanvas(width: number, height: number): { canvas: Canvas; ctx: CanvasRenderingContext2D } | null {
  const canvas = document.createElement("canvas");
  canvas.width = width;
  canvas.height = height;
  const ctx = canvas.getContext("2d");
  return ctx ? { canvas, ctx } : null;
}

function supportsFilter(ctx: CanvasRenderingContext2D): boolean {
  return typeof ctx.filter === "string";
}

/** Copies `src` into a larger canvas with its edge pixels stretched outwards by `m` px, so that a blur does not fade at the edges. */
function withExtendedEdges(src: Canvas, m: number): { canvas: Canvas; ctx: CanvasRenderingContext2D } | null {
  const w = src.width;
  const h = src.height;
  const big = newCanvas(w + 2 * m, h + 2 * m);
  if (!big) return null;
  const { ctx } = big;
  ctx.drawImage(src, m, m);
  ctx.drawImage(src, 0, 0, w, 1, m, 0, w, m); // top
  ctx.drawImage(src, 0, h - 1, w, 1, m, m + h, w, m); // bottom
  ctx.drawImage(src, 0, 0, 1, h, 0, m, m, h); // left
  ctx.drawImage(src, w - 1, 0, 1, h, m + w, m, m, h); // right
  ctx.drawImage(src, 0, 0, 1, 1, 0, 0, m, m);
  ctx.drawImage(src, w - 1, 0, 1, 1, m + w, 0, m, m);
  ctx.drawImage(src, 0, h - 1, 1, 1, 0, m + h, m, m);
  ctx.drawImage(src, w - 1, h - 1, 1, 1, m + w, m + h, m, m);
  return big;
}

/** `src` drawn with `filter` (a CSS filter list) and nothing lost at the edges; `null` when the canvas has no filters. */
function filtered(src: Canvas, filter: string, blurPx: number): Canvas | null {
  const margin = Math.ceil(blurPx * 3);
  const big = withExtendedEdges(src, margin);
  const out = newCanvas(src.width, src.height);
  if (!big || !out || !supportsFilter(out.ctx)) return null;
  out.ctx.filter = filter;
  out.ctx.drawImage(big.canvas, -margin, -margin);
  out.ctx.filter = "none";
  return out.canvas;
}

/** The gradients of DESIGN.md 1.5 painted by `createRadialGradient` (an ellipse is a circle under a scale), then dimmed. */
function paintGradients(ctx: CanvasRenderingContext2D, w: number, h: number, id: GradientWallpaper): void {
  ctx.fillStyle = WALLPAPER_BASE;
  ctx.fillRect(0, 0, w, h);
  // CSS draws the first layer on top: paint from the last.
  for (const layer of [...WALLPAPER_LAYERS[id]].reverse()) {
    ctx.save();
    ctx.translate(layer.cx * w, layer.cy * h);
    ctx.scale(layer.rx * w, layer.ry * h);
    const g = ctx.createRadialGradient(0, 0, 0, 0, 0, 1);
    const rgb = layer.color.slice(1).match(/../g)!.map((x) => parseInt(x, 16));
    g.addColorStop(0, layer.color);
    // `transparent` of CSS means the same colour at alpha 0 (premultiplied interpolation): no dark fringe.
    g.addColorStop(layer.end, `rgba(${rgb[0]},${rgb[1]},${rgb[2]},0)`);
    ctx.fillStyle = g;
    ctx.fillRect(-layer.cx / layer.rx, -layer.cy / layer.ry, 1 / layer.rx, 1 / layer.ry);
    ctx.restore();
  }
  ctx.fillStyle = `rgba(0,0,0,${WALLPAPER_DIM})`;
  ctx.fillRect(0, 0, w, h);
}

/** The cover scaled 1.3x about the centre, `cover`-fitted (the central square, as the keys' album art). */
function paintCover(ctx: CanvasRenderingContext2D, w: number, h: number, img: CanvasImageSource, iw: number, ih: number): void {
  const k = Math.max(w / iw, h / ih) * 1.3;
  const dw = iw * k;
  const dh = ih * k;
  ctx.fillStyle = WALLPAPER_BASE;
  ctx.fillRect(0, 0, w, h);
  ctx.drawImage(img, (w - dw) / 2, (h - dh) / 2, dw, dh);
}

function noisy(canvas: Canvas, seed: number): void {
  const ctx = canvas.getContext("2d");
  if (!ctx) return;
  const data = ctx.getImageData(0, 0, canvas.width, canvas.height);
  addNoise(data.data, NOISE_AMPLITUDE, seed);
  ctx.putImageData(data, 0, 0);
}

function loadImage(src: string): Promise<HTMLImageElement | null> {
  return new Promise((resolve) => {
    const img = new Image();
    img.onload = () => resolve(img);
    img.onerror = () => resolve(null);
    img.src = src;
  });
}

/**
 * "Background 1": the wallpaper pre-blurred into a canvas of a quarter of the screen. A gradient is painted and dimmed,
 * then blurred; the cover of `album` is blurred with `brightness(0.55) saturate(140%)`. Noise of +-3 hides banding.
 * `null` when the canvas cannot be used (the caller keeps the CSS gradients).
 */
export async function renderWallpaper(id: WallpaperId, art: string | undefined, size: { width: number; height: number }): Promise<Canvas | null> {
  const { width: w, height: h } = size;
  const blur = scaledBlur(BASE_BLUR_PX, w);
  const base = newCanvas(w, h);
  if (!base) return null;
  let result: Canvas | null;
  if (id === "album" && art !== undefined) {
    const img = await loadImage(art);
    if (!img) return null;
    paintCover(base.ctx, w, h, img, img.naturalWidth, img.naturalHeight);
    result = filtered(base.canvas, `blur(${blur}px) brightness(0.55) saturate(140%)`, blur);
    if (!result) {
      // No canvas filters: blur by drawing through a tiny canvas, darken by hand.
      result = coarseBlur(base.canvas);
      const ctx = result?.getContext("2d");
      if (ctx) {
        ctx.fillStyle = "rgba(0,0,0,0.45)";
        ctx.fillRect(0, 0, w, h);
      }
    }
  } else {
    const gradient: GradientWallpaper = id === "album" ? "aurora" : id;
    paintGradients(base.ctx, w, h, gradient);
    result = filtered(base.canvas, `blur(${blur}px)`, blur) ?? base.canvas; // the gradients are smooth by themselves
  }
  if (!result) return null;
  noisy(result, 7);
  return result;
}

/** Blur without canvas filters: down to 1/8 and back up with smoothing. Coarse, only a fallback. */
function coarseBlur(src: Canvas): Canvas | null {
  const small = newCanvas(Math.max(2, Math.round(src.width / 8)), Math.max(2, Math.round(src.height / 8)));
  const out = newCanvas(src.width, src.height);
  if (!small || !out) return null;
  small.ctx.drawImage(src, 0, 0, small.canvas.width, small.canvas.height);
  out.ctx.imageSmoothingQuality = "high";
  out.ctx.drawImage(small.canvas, 0, 0, out.canvas.width, out.canvas.height);
  return out.canvas;
}

/**
 * "Background 2" for `reduced`: the part of "background 1" the screen shows, blurred 6 px more and saturated 160%, at
 * screen proportions. Each key gets it as a `background-image` the size of the screen (DESIGN.md 1.4).
 */
export function renderFrost(base: Canvas): Canvas | null {
  const crop = screenCrop(base.width, base.height);
  const part = newCanvas(Math.round(crop.sw), Math.round(crop.sh));
  if (!part) return null;
  part.ctx.drawImage(base, crop.sx, crop.sy, crop.sw, crop.sh, 0, 0, part.canvas.width, part.canvas.height);
  const blur = scaledBlur(FROST_BLUR_PX, base.width);
  const frosted = filtered(part.canvas, `blur(${blur}px) saturate(160%)`, blur);
  if (frosted) return frosted;
  return coarseBlur(part.canvas) ?? part.canvas;
}

/** `canvas.toBlob` -> object URL (`null` when the browser cannot encode it). */
export function canvasToObjectUrl(canvas: Canvas): Promise<string | null> {
  return new Promise((resolve) => {
    try {
      canvas.toBlob((blob) => resolve(blob ? URL.createObjectURL(blob) : null), "image/png");
    } catch {
      resolve(null);
    }
  });
}
