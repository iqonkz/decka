/** The manual choice of a shortcut (DATA_MODEL.md 10.3, DESIGN.md 4.9): four modifier switches and one key. */

export const MODIFIERS = ["ctrl", "alt", "shift", "win"] as const;
export type Modifier = (typeof MODIFIERS)[number];

export interface ChordState {
  ctrl: boolean;
  alt: boolean;
  shift: boolean;
  win: boolean;
  /** The main key (`m`, `f5`, `printscreen`...), `null` while none is chosen. */
  key: string | null;
}

const isModifier = (name: string): name is Modifier => (MODIFIERS as readonly string[]).includes(name);

/** `keys` of an action as the switches and the key; the first non-modifier key is the main one. */
export function chordToState(keys: readonly string[]): ChordState {
  return {
    ctrl: keys.includes("ctrl"),
    alt: keys.includes("alt"),
    shift: keys.includes("shift"),
    win: keys.includes("win"),
    key: keys.find((k) => !isModifier(k)) ?? null,
  };
}

/**
 * The `keys` of the state: modifiers in the order `ctrl`, `alt`, `shift`, `win`, then the key. Without a key it is an error
 * (`Pick a key`), except a lone `win`, which opens the Start menu.
 */
export function stateToChord(state: ChordState): { ok: true; keys: string[] } | { ok: false; error: "pick_key" } {
  const mods = MODIFIERS.filter((m) => state[m]);
  if (state.key === null) {
    if (mods.length === 1 && mods[0] === "win") return { ok: true, keys: ["win"] };
    return { ok: false, error: "pick_key" };
  }
  return { ok: true, keys: [...mods, state.key] };
}

export interface KeyGroup {
  id: "letters" | "digits" | "fkeys" | "nav" | "symbols";
  keys: string[];
}

const range = (n: number, make: (i: number) => string) => Array.from({ length: n }, (_, i) => make(i));

/** Every main key of DATA_MODEL.md 2.2 that is not a modifier, in groups. */
export const KEY_GROUPS: KeyGroup[] = [
  { id: "letters", keys: range(26, (i) => String.fromCharCode(97 + i)) },
  { id: "digits", keys: range(10, (i) => String(i)) },
  { id: "fkeys", keys: range(24, (i) => `f${i + 1}`) },
  {
    id: "nav",
    keys: ["up", "down", "left", "right", "home", "end", "pageup", "pagedown", "insert", "delete", "backspace", "enter", "esc", "tab", "space", "printscreen"],
  },
  { id: "symbols", keys: ["period", "comma", "minus", "equal", "slash", "semicolon"] },
];

const LABELS: Record<string, string> = {
  up: "Up",
  down: "Down",
  left: "Left",
  right: "Right",
  home: "Home",
  end: "End",
  pageup: "Page Up",
  pagedown: "Page Down",
  insert: "Insert",
  delete: "Delete",
  backspace: "Backspace",
  enter: "Enter",
  esc: "Esc",
  tab: "Tab",
  space: "Space",
  printscreen: "Print Screen",
  period: ".",
  comma: ",",
  minus: "-",
  equal: "=",
  slash: "/",
  semicolon: ";",
};

/** The caption of a key name: `Print Screen`, `Page Up`, `.`, `F5`, `M`. */
export function keyLabel(name: string): string {
  return LABELS[name] ?? name.toUpperCase();
}

/** The keys whose caption or name contains `query` (case-insensitive); an empty query keeps every key. */
export function searchKeys(query: string): KeyGroup[] {
  const q = query.trim().toLowerCase();
  if (q === "") return KEY_GROUPS;
  return KEY_GROUPS.map((g) => ({ id: g.id, keys: g.keys.filter((k) => k.includes(q) || keyLabel(k).toLowerCase().includes(q)) })).filter((g) => g.keys.length > 0);
}
