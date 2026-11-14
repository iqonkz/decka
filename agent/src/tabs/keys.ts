// Key names allowed in hotkey actions (DATA_MODEL.md 2.2).
const MODIFIERS = ["ctrl", "alt", "shift", "win"];
const NAMED = ["enter", "esc", "space", "tab", "backspace", "delete", "home", "end", "pageup", "pagedown", "up", "down", "left", "right"];

export function isKeyName(name: string): boolean {
  if (MODIFIERS.includes(name) || NAMED.includes(name)) return true;
  if (/^[a-z0-9]$/.test(name)) return true;
  const f = /^f([1-9]|1[0-9]|2[0-4])$/.exec(name);
  return f !== null;
}

/**
 * "Ctrl + Shift + M" -> ["ctrl", "shift", "m"]. Returns null when a part is not an allowed key,
 * a key repeats, or there are not 1..5 keys.
 */
export function parseKeys(input: string): string[] | null {
  const keys = input.split("+").map((part) => part.trim().toLowerCase());
  if (keys.length < 1 || keys.length > 5) return null;
  if (!keys.every(isKeyName)) return null;
  if (new Set(keys).size !== keys.length) return null;
  return keys;
}

export function formatKeys(keys: string[]): string {
  return keys.join("+");
}
