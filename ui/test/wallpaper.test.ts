import { describe, expect, test } from "vitest";
import { addNoise, bitmapSize, frostPosition, LAYER_SCALE, MAX_SIDE, NOISE_AMPLITUDE, scaledBlur, screenCrop, seeded, WALLPAPER_LAYERS } from "../src/wallpaper";

describe("bitmapSize: a quarter of the screen in device pixels", () => {
  test("A57: a 2340x1080 screen (layer 116%) gives about 585x270", () => {
    // 780x360 CSS px at dpr 3 is 2340x1080 device px.
    const s = bitmapSize(780 * LAYER_SCALE, 360 * LAYER_SCALE, 3);
    expect(s.width).toBe(585);
    expect(s.height).toBe(270);
  });
  test("a big screen is capped, a tiny preview is lifted, proportions stay", () => {
    const big = bitmapSize(4000, 2000, 2);
    expect(Math.max(big.width, big.height)).toBe(MAX_SIDE);
    expect(big.width / big.height).toBeCloseTo(2, 1);
    const tiny = bitmapSize(40, 20, 1);
    expect(tiny.width).toBeGreaterThanOrEqual(64);
    expect(tiny.width / tiny.height).toBeCloseTo(2, 1);
  });
});

describe("blur radius follows the canvas width", () => {
  test("12 px at the A57 canvas, half of it at half the width", () => {
    expect(scaledBlur(12, 585)).toBe(12);
    expect(scaledBlur(12, 292.5)).toBe(6);
    expect(scaledBlur(6, 1)).toBeGreaterThan(0);
  });
});

describe("noise against banding", () => {
  test("every colour channel moves by at most 3 and alpha is untouched", () => {
    const px = new Uint8ClampedArray(4 * 5000).fill(128);
    for (let i = 3; i < px.length; i += 4) px[i] = 200;
    addNoise(px);
    let moved = 0;
    for (let i = 0; i < px.length; i += 4) {
      for (let c = 0; c < 3; c++) expect(Math.abs((px[i + c] as number) - 128)).toBeLessThanOrEqual(NOISE_AMPLITUDE);
      expect(px[i + 3]).toBe(200);
      if (px[i] !== 128) moved++;
    }
    expect(moved).toBeGreaterThan(2000); // 6 of 7 values move the pixel
  });
  test("the same seed gives the same noise; the mean brightness stays", () => {
    const a = new Uint8ClampedArray(4 * 4000).fill(100);
    const b = new Uint8ClampedArray(4 * 4000).fill(100);
    addNoise(a, 3, 9);
    addNoise(b, 3, 9);
    expect(Array.from(a)).toEqual(Array.from(b));
    let sum = 0;
    for (let i = 0; i < a.length; i += 4) sum += a[i] as number;
    expect(sum / 4000).toBeGreaterThan(99.5);
    expect(sum / 4000).toBeLessThan(100.5);
  });
  test("the generator is in [0, 1)", () => {
    const next = seeded(5);
    for (let i = 0; i < 1000; i++) {
      const v = next();
      expect(v).toBeGreaterThanOrEqual(0);
      expect(v).toBeLessThan(1);
    }
  });
});

describe("frostPosition: the screen-sized frosted picture moved by minus the key's position", () => {
  const viewport = { width: 780, height: 360 };
  test("a key at (100, 50)", () => {
    expect(frostPosition({ left: 100, top: 50, width: 80, height: 80 }, viewport)).toEqual({ x: -100, y: -50, width: 780, height: 360 });
  });
  test("the key at the corner shows the corner of the picture", () => {
    const p = frostPosition({ left: 0, top: 0, width: 80, height: 80 }, viewport);
    expect(p.x === 0 && p.y === 0).toBe(true);
  });
  test("two keys see two different pieces; neighbours differ by exactly the key pitch", () => {
    const a = frostPosition({ left: 12, top: 12, width: 80, height: 80 }, viewport);
    const b = frostPosition({ left: 104.5, top: 12, width: 80, height: 80 }, viewport);
    expect(a.x - b.x).toBeCloseTo(92.5, 5);
    expect(a.y).toBe(b.y);
  });
  test("the piece under the key is the same piece of the picture: key position + picture offset = 0", () => {
    const rect = { left: 333.25, top: 91.5, width: 96, height: 96 };
    const p = frostPosition(rect, viewport);
    expect(rect.left + p.x).toBeCloseTo(0, 1);
    expect(rect.top + p.y).toBeCloseTo(0, 1);
  });
});

describe("the part of the wallpaper the screen shows", () => {
  test("the central 1/1.16 of the canvas", () => {
    const c = screenCrop(580, 270);
    expect(c.sw).toBeCloseTo(500, 5);
    expect(c.sh).toBeCloseTo(232.76, 1);
    expect(c.sx).toBeCloseTo(40, 5);
    expect(c.sx * 2 + c.sw).toBeCloseTo(580, 5);
  });
});

describe("gradient data of DESIGN.md 1.5", () => {
  test("every wallpaper but album has layers with sane geometry", () => {
    for (const [name, layers] of Object.entries(WALLPAPER_LAYERS)) {
      expect(layers.length, name).toBeGreaterThanOrEqual(2);
      for (const l of layers) {
        expect(l.color, name).toMatch(/^#[0-9a-f]{6}$/);
        expect(l.rx).toBeGreaterThan(0);
        expect(l.ry).toBeGreaterThan(0);
        expect(l.end).toBeGreaterThan(0);
        expect(l.end).toBeLessThanOrEqual(1);
      }
    }
  });
  test("the colour stops are the ones the contrast test checks", () => {
    expect(WALLPAPER_LAYERS.aurora.map((l) => l.color)).toEqual(["#6d28d9", "#0ea5e9", "#10b981"]);
    expect(WALLPAPER_LAYERS.sunset.map((l) => l.color)).toEqual(["#f97316", "#db2777", "#7c3aed"]);
    expect(WALLPAPER_LAYERS.ocean.map((l) => l.color)).toEqual(["#1d4ed8", "#0891b2", "#0f766e"]);
    expect(WALLPAPER_LAYERS.graphite.map((l) => l.color)).toEqual(["#3f3f46", "#27272a"]);
  });
});
