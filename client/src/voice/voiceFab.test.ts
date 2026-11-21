import { describe, it, expect } from "vitest";
import {
  snapToEdge,
  detectVoiceGesture,
  DRAG_SLOP_PX,
  HOLD_TO_TALK_THRESHOLD_MS,
  loadFabPosition,
  saveFabPosition,
  FAB_STORAGE_KEY,
} from "./voiceFabMath";

describe("Voice FAB math and gesture detection (T21.5b)", () => {
  const viewport = { width: 800, height: 480 };
  const size = 56;
  const margin = 16;

  describe("snapToEdge", () => {
    it("snaps to left edge when center is on left half", () => {
      // center x = 100 + 28 = 128 < 400
      const pos = { x: 100, y: 200 };
      const snapped = snapToEdge(pos, viewport, size, margin);
      expect(snapped.x).toBe(margin); // 16
      expect(snapped.y).toBe(200);
    });

    it("snaps to right edge when center is on right half", () => {
      // center x = 500 + 28 = 528 > 400
      const pos = { x: 500, y: 150 };
      const snapped = snapToEdge(pos, viewport, size, margin);
      expect(snapped.x).toBe(viewport.width - size - margin); // 800 - 56 - 16 = 728
      expect(snapped.y).toBe(150);
    });

    it("clamps y within top and bottom margins", () => {
      const tooHigh = { x: 100, y: -20 };
      const snappedTop = snapToEdge(tooHigh, viewport, size, margin);
      expect(snappedTop.y).toBe(margin); // 16

      const tooLow = { x: 700, y: 600 };
      const snappedBottom = snapToEdge(tooLow, viewport, size, margin);
      expect(snappedBottom.y).toBe(viewport.height - size - margin); // 480 - 56 - 16 = 408
    });
  });

  describe("detectVoiceGesture threshold (250 ms, 8 px)", () => {
    it("returns 'pending' when within slop and time < 250ms", () => {
      expect(detectVoiceGesture(0, 0, 0)).toBe("pending");
      expect(detectVoiceGesture(4, 4, 100)).toBe("pending"); // hypot(4,4) ≈ 5.66 <= 8
      expect(detectVoiceGesture(0, 7, 240)).toBe("pending");
    });

    it("returns 'talk' when elapsed time >= 250ms and distance <= 8 px", () => {
      expect(detectVoiceGesture(0, 0, 250)).toBe("talk");
      expect(detectVoiceGesture(5, 5, 300)).toBe("talk"); // hypot(5,5) ≈ 7.07 <= 8
      expect(detectVoiceGesture(0, 8, HOLD_TO_TALK_THRESHOLD_MS)).toBe("talk");
    });

    it("returns 'drag' as soon as movement exceeds 8 px regardless of time", () => {
      expect(detectVoiceGesture(9, 0, 50)).toBe("drag");
      expect(detectVoiceGesture(0, 10, 100)).toBe("drag");
      expect(detectVoiceGesture(6, 6, 200)).toBe("drag"); // hypot(6,6) ≈ 8.49 > 8
      expect(detectVoiceGesture(15, 20, 500)).toBe("drag");
    });
  });

  describe("persistence", () => {
    it("saves and loads position from storage", () => {
      const store = new Map<string, string>();
      const mockStorage = {
        getItem: (k: string) => store.get(k) ?? null,
        setItem: (k: string, v: string) => {
          store.set(k, v);
        },
        removeItem: (k: string) => {
          store.delete(k);
        },
        clear: () => store.clear(),
        key: () => null,
        length: 0,
      } as Storage;
      globalThis.localStorage = mockStorage;

      const pos = { x: 16, y: 120 };
      saveFabPosition(pos);
      const loaded = loadFabPosition(viewport, size, margin);
      expect(loaded.x).toBe(16);
      expect(loaded.y).toBe(120);
    });
  });
});
