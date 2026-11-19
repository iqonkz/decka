/** The parts of a `KeyboardEvent` the shortcut recorder reads (a plain object works in tests). */
export interface KeyEventLike {
  key: string;
  code: string;
  ctrlKey?: boolean;
  altKey?: boolean;
  shiftKey?: boolean;
  metaKey?: boolean;
}

const NAMED: Record<string, string> = {
  Enter: "enter",
  NumpadEnter: "enter",
  Escape: "esc",
  Space: "space",
  Tab: "tab",
  Backspace: "backspace",
  Delete: "delete",
  Home: "home",
  End: "end",
  PageUp: "pageup",
  PageDown: "pagedown",
  ArrowUp: "up",
  ArrowDown: "down",
  ArrowLeft: "left",
  ArrowRight: "right",
  PrintScreen: "printscreen",
  Insert: "insert",
  Period: "period",
  Comma: "comma",
  Minus: "minus",
  Equal: "equal",
  Slash: "slash",
  Semicolon: "semicolon",
};

const MODIFIER_KEYS: Record<string, string> = { Control: "ctrl", Alt: "alt", AltGraph: "alt", Shift: "shift", Meta: "win", OS: "win" };

/**
 * The name of the key of this event in the set of DATA_MODEL.md 2.2 (`ctrl`, `m`, `5`, `f12`, `left`...), or `null` when
 * the key is not supported. Letters and digits come from `code` (the physical key), so a Russian layout still gives `m`.
 */
export function keyNameFromEvent(e: KeyEventLike): string | null {
  const modifier = MODIFIER_KEYS[e.key];
  if (modifier) return modifier;
  const letter = /^Key([A-Z])$/.exec(e.code);
  if (letter) return letter[1]!.toLowerCase();
  const digit = /^(?:Digit|Numpad)([0-9])$/.exec(e.code);
  if (digit) return digit[1]!;
  const f = /^F([1-9]|1[0-9]|2[0-4])$/.exec(e.code) ?? /^F([1-9]|1[0-9]|2[0-4])$/.exec(e.key);
  if (f) return `f${f[1]}`;
  const named = NAMED[e.code] ?? NAMED[e.key];
  if (named) return named;
  // Input without a physical key code (remote desktop, some virtual keyboards): a Latin letter or digit by its character.
  return /^[a-zA-Z0-9]$/.test(e.key) ? e.key.toLowerCase() : null;
}

const ORDER = ["ctrl", "alt", "shift", "win"];

/**
 * The chord an event makes: modifiers held (in a fixed order) and the main key. `unsupported` when the main key is not in
 * the set; `keys` is then the modifiers only. A modifier pressed alone is a chord of one modifier.
 */
export function chordFromEvent(e: KeyEventLike): { keys: string[]; unsupported: boolean } {
  const held = [e.ctrlKey && "ctrl", e.altKey && "alt", e.shiftKey && "shift", e.metaKey && "win"].filter((k): k is string => typeof k === "string");
  const main = keyNameFromEvent(e);
  if (main === null) return { keys: held, unsupported: true };
  if (ORDER.includes(main)) return { keys: held.includes(main) ? held : [...held, main].sort((a, b) => ORDER.indexOf(a) - ORDER.indexOf(b)), unsupported: false };
  return { keys: [...held, main], unsupported: false };
}
