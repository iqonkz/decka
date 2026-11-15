import { describe, expect, test } from "vitest";
import { chordFromEvent, keyNameFromEvent } from "../src/editor/keyName";
import { isKeyName } from "../src/tabs/keys";

const ev = (key: string, code: string, flags: Partial<Record<"ctrlKey" | "altKey" | "shiftKey" | "metaKey", boolean>> = {}) => ({ key, code, ...flags });

describe("keyNameFromEvent", () => {
  test("modifiers", () => {
    expect(keyNameFromEvent(ev("Control", "ControlLeft"))).toBe("ctrl");
    expect(keyNameFromEvent(ev("Shift", "ShiftRight"))).toBe("shift");
    expect(keyNameFromEvent(ev("Alt", "AltLeft"))).toBe("alt");
    expect(keyNameFromEvent(ev("Meta", "MetaLeft"))).toBe("win");
  });
  test("letters come from the physical key, so another layout still works", () => {
    expect(keyNameFromEvent(ev("m", "KeyM"))).toBe("m");
    expect(keyNameFromEvent(ev("M", "KeyM", { shiftKey: true }))).toBe("m");
    expect(keyNameFromEvent(ev("ь", "KeyM"))).toBe("m");
  });
  test("without a physical code a Latin letter or digit is taken by its character", () => {
    expect(keyNameFromEvent(ev("M", ""))).toBe("m");
    expect(keyNameFromEvent(ev("7", ""))).toBe("7");
    expect(keyNameFromEvent(ev("ь", ""))).toBeNull();
  });
  test("digits, also on the numpad", () => {
    expect(keyNameFromEvent(ev("5", "Digit5"))).toBe("5");
    expect(keyNameFromEvent(ev("7", "Numpad7"))).toBe("7");
  });
  test("F1..F24", () => {
    for (let n = 1; n <= 24; n++) expect(keyNameFromEvent(ev(`F${n}`, `F${n}`))).toBe(`f${n}`);
    expect(keyNameFromEvent(ev("F25", "F25"))).toBeNull();
  });
  test("arrows and the named keys", () => {
    expect(keyNameFromEvent(ev("ArrowUp", "ArrowUp"))).toBe("up");
    expect(keyNameFromEvent(ev("ArrowLeft", "ArrowLeft"))).toBe("left");
    expect(keyNameFromEvent(ev("Escape", "Escape"))).toBe("esc");
    expect(keyNameFromEvent(ev(" ", "Space"))).toBe("space");
    expect(keyNameFromEvent(ev("PageDown", "PageDown"))).toBe("pagedown");
    expect(keyNameFromEvent(ev("Enter", "Enter"))).toBe("enter");
  });
  test("everything it returns is a key of DATA_MODEL.md 2.2; unsupported keys give null", () => {
    for (const e of [ev("Control", "ControlLeft"), ev("m", "KeyM"), ev("F12", "F12"), ev("ArrowDown", "ArrowDown"), ev("5", "Digit5")]) expect(isKeyName(keyNameFromEvent(e)!)).toBe(true);
    expect(keyNameFromEvent(ev(";", "Semicolon"))).toBeNull();
    expect(keyNameFromEvent(ev("CapsLock", "CapsLock"))).toBeNull();
    expect(keyNameFromEvent(ev("Insert", "Insert"))).toBeNull();
    expect(keyNameFromEvent(ev("PrintScreen", "PrintScreen"))).toBeNull();
  });
});

describe("chordFromEvent", () => {
  test("Ctrl+Shift+M: modifiers first, in a fixed order", () => {
    expect(chordFromEvent(ev("M", "KeyM", { ctrlKey: true, shiftKey: true }))).toEqual({ keys: ["ctrl", "shift", "m"], unsupported: false });
    expect(chordFromEvent(ev("m", "KeyM", { metaKey: true, altKey: true, ctrlKey: true }))).toEqual({ keys: ["ctrl", "alt", "win", "m"], unsupported: false });
  });
  test("a modifier alone is a chord of that modifier", () => {
    expect(chordFromEvent(ev("Control", "ControlLeft", { ctrlKey: true }))).toEqual({ keys: ["ctrl"], unsupported: false });
    expect(chordFromEvent(ev("Shift", "ShiftLeft", { ctrlKey: true, shiftKey: true }))).toEqual({ keys: ["ctrl", "shift"], unsupported: false });
  });
  test("an unsupported key keeps the modifiers and says so", () => {
    expect(chordFromEvent(ev(";", "Semicolon", { ctrlKey: true }))).toEqual({ keys: ["ctrl"], unsupported: true });
  });
});
