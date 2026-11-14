import type { ButtonItem, ButtonLook, VarValue } from "@decka/shared";

/** Base look of the button with `bind.states[String(var)]` laid over it (DATA_MODEL.md 3.4). */
export function resolveLook(item: ButtonItem, vars: Record<string, VarValue> | undefined): ButtonLook {
  const base: ButtonLook = {};
  if (item.label !== undefined) base.label = item.label;
  if (item.icon !== undefined) base.icon = item.icon;
  if (item.color !== undefined) base.color = item.color;
  if (!item.bind) return base;

  const value = vars?.[item.bind.var];
  const key = value === undefined ? "false" : String(value);
  const override = item.bind.states[key];
  return override ? { ...base, ...definedOnly(override) } : base;
}

function definedOnly(look: ButtonLook): ButtonLook {
  const out: ButtonLook = {};
  if (look.label !== undefined) out.label = look.label;
  if (look.icon !== undefined) out.icon = look.icon;
  if (look.color !== undefined) out.color = look.color;
  return out;
}

const LIGHT = "#e5e5e5";

function parseHex(color: string): [number, number, number] | null {
  const m = /^#([0-9a-fA-F]{6})$/.exec(color) ?? /^#([0-9a-fA-F]{3})$/.exec(color);
  if (!m || !m[1]) return null;
  const h = m[1].length === 3 ? [...m[1]].map((c) => c + c).join("") : m[1];
  return [parseInt(h.slice(0, 2), 16), parseInt(h.slice(2, 4), 16), parseInt(h.slice(4, 6), 16)];
}

export interface KeyColors {
  /** Key background; transparent without a button color. */
  background: string;
  /** Key background while pressed. */
  pressed: string;
  /** Icon / glyph color. */
  ink: string;
}

/** T10.2: a coloured key is a 16% tint of the colour with the icon in that colour (light ink when it is too dark). */
export function keyColors(color: string | undefined): KeyColors {
  const rgb = color ? parseHex(color) : null;
  if (!rgb || !color) return { background: "transparent", pressed: "rgba(255,255,255,0.10)", ink: LIGHT };
  const [r, g, b] = rgb;
  const luminance = 0.2126 * r + 0.7152 * g + 0.0722 * b;
  return {
    background: `rgba(${r},${g},${b},0.16)`,
    pressed: `rgba(${r},${g},${b},0.30)`,
    ink: luminance < 64 ? LIGHT : color,
  };
}

/** Key side for a cell: 62% of the smaller cell side. */
export function keySize(cellW: number, cellH: number): number {
  return Math.round(0.62 * Math.min(cellW, cellH));
}

/** Toast title for a failed press, by error code. */
export function pressErrorTitle(e: { code: string; message?: string }): string {
  switch (e.code) {
    case "UNKNOWN_ACTION":
      return "Action is not defined on the PC";
    case "ACTION_FAILED":
      return "Action failed on the PC";
    case "RATE_LIMITED":
      return "Too fast";
    case "TIMEOUT":
      return "PC did not respond";
    case "OFFLINE":
      return "Not connected";
    default:
      return e.message || e.code;
  }
}
