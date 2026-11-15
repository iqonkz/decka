import { describe, expect, test, vi } from "vitest";
import { INTERACTIVE_SELECTOR, isEmptySpot, MENU_HOLD_MS, statusLine } from "../src/menu/menuModel";

describe("statusLine", () => {
  test("connected over USB or Wi-Fi", () => {
    expect(statusLine("online", "usb", "DESKTOP-MAIN")).toBe("Connected to DESKTOP-MAIN · USB");
    expect(statusLine("online", "lan", "DESKTOP-MAIN")).toBe("Connected to DESKTOP-MAIN · Wi-Fi");
  });

  test("the route may be unknown, the name too", () => {
    expect(statusLine("online", null, "PC")).toBe("Connected to PC");
    expect(statusLine("online", "lan", null)).toBe("Connected to your PC · Wi-Fi");
  });

  test("anything but online is Offline", () => {
    for (const conn of ["connecting", "offline", "unpaired", "update_required"] as const) expect(statusLine(conn, "lan", "PC"), conn).toBe("Offline");
  });
});

describe("the hold", () => {
  test("lasts 1.2 s (DESIGN.md 4.3)", () => {
    expect(MENU_HOLD_MS).toBe(1200);
  });

  test("only empty space counts: not a key, a widget, a group, a toast, a dialog or the menu itself", () => {
    // The Node test environment has no DOM: a small stand-in for `Element` with a `closest` that knows what is "inside".
    class FakeElement {
      constructor(private readonly insideInteractive: boolean) {}
      closest(selector: string) {
        return selector === INTERACTIVE_SELECTOR && this.insideInteractive ? this : null;
      }
    }
    vi.stubGlobal("Element", FakeElement);
    try {
      expect(isEmptySpot(new FakeElement(false) as unknown as EventTarget)).toBe(true);
      expect(isEmptySpot(new FakeElement(true) as unknown as EventTarget)).toBe(false);
      expect(isEmptySpot(null)).toBe(false);
    } finally {
      vi.unstubAllGlobals();
    }
    // Without a DOM at all, nothing is an empty spot.
    expect(isEmptySpot({} as EventTarget)).toBe(false);
  });

  test("the selector lists the interactive parts", () => {
    for (const part of [".key", ".tile", ".popup-backdrop", ".toasts", ".dialog-backdrop", ".menu-sheet"]) expect(INTERACTIVE_SELECTOR).toContain(part);
  });
});
