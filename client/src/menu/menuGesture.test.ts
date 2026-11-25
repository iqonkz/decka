import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { attachMenuGesture, type GestureHost } from "./useMenuGesture";
import { MENU_HOLD_MS } from "./menuModel";
import { HOLD_TO_TALK_THRESHOLD_MS } from "../voice/voiceFabMath";

// A tiny DOM stand-in: an element knows its ancestors and answers `closest` by class / attribute selectors.
class FakeElement {
  constructor(
    private classes: string[],
    private attrs: string[],
    private parent: FakeElement | null = null,
  ) {}
  closest(selector: string): FakeElement | null {
    const parts = selector.split(",").map((s) => s.trim());
    for (let el: FakeElement | null = this; el; el = el.parent) {
      for (const p of parts) {
        if (p.startsWith(".") && el.classes.includes(p.slice(1))) return el;
        if (p.startsWith("[") && el.attrs.includes(p.slice(1, -1))) return el;
      }
    }
    return null;
  }
}

function setup() {
  const handlers = new Map<string, (e: PointerEvent) => void>();
  const host: GestureHost = {
    addEventListener: (type, fn) => void handlers.set(type, fn),
    removeEventListener: (type) => void handlers.delete(type),
    setTimeout: (fn, ms) => setTimeout(fn, ms) as unknown as number,
    clearTimeout: (id) => clearTimeout(id),
  };
  const onOpen = vi.fn();
  attachMenuGesture(host, onOpen, () => {});
  const down = (target: FakeElement) => handlers.get("pointerdown")!({ target, clientX: 10, clientY: 10 } as unknown as PointerEvent);
  return { onOpen, down };
}

describe("phone menu hold (T25.1)", () => {
  beforeEach(() => {
    vi.stubGlobal("Element", FakeElement);
    vi.useFakeTimers();
  });
  afterEach(() => {
    vi.useRealTimers();
    vi.unstubAllGlobals();
  });

  it("holding the voice tile 1.5 s does not open the menu (the voice hold starts first)", () => {
    const { onOpen, down } = setup();
    const root = new FakeElement(["deck"], []);
    const fab = new FakeElement(["voice-fab"], ["data-tile"], root);
    const icon = new FakeElement(["voice-fab-icon"], [], fab); // the touch lands on the icon inside the tile
    down(icon);
    expect(HOLD_TO_TALK_THRESHOLD_MS).toBeLessThan(MENU_HOLD_MS);
    vi.advanceTimersByTime(1500);
    expect(onOpen).not.toHaveBeenCalled();
  });

  it("any element inside [data-tile] cancels the hold, without relying on a class", () => {
    const { onOpen, down } = setup();
    const tile = new FakeElement([], ["data-tile"]);
    down(new FakeElement(["some-widget-inner"], [], tile));
    vi.advanceTimersByTime(3000);
    expect(onOpen).not.toHaveBeenCalled();
  });

  it("holding empty space still opens the menu", () => {
    const { onOpen, down } = setup();
    down(new FakeElement(["deck"], []));
    vi.advanceTimersByTime(MENU_HOLD_MS + 10);
    expect(onOpen).toHaveBeenCalledTimes(1);
  });
});
