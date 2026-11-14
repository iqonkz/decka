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

function parseHex(color: string): [number, number, number] | null {
  const m = /^#([0-9a-fA-F]{6})$/.exec(color) ?? /^#([0-9a-fA-F]{3})$/.exec(color);
  if (!m || !m[1]) return null;
  const h = m[1].length === 3 ? [...m[1]].map((c) => c + c).join("") : m[1];
  return [parseInt(h.slice(0, 2), 16), parseInt(h.slice(2, 4), 16), parseInt(h.slice(4, 6), 16)];
}

export interface KeyFace {
  /** CSS background of the key face (a vertical gradient). */
  background: string;
  /** Icon / glyph colour. */
  ink: string;
  /** Colour of the 1 px outline. */
  border: string;
}

/** The face of an uncoloured key, also used by widget panels. */
export const PLAIN_FACE: KeyFace = {
  background: "linear-gradient(180deg, #1c1c22 0%, #0d0d10 100%)",
  ink: "#f2f2f2",
  border: "rgba(255,255,255,0.10)",
};

const mix = (c: number, target: number, amount: number): number => Math.round(c + (target - c) * amount);

/** `color` mixed with white (`amount` > 0) or black (`amount` < 0) in sRGB, as `r,g,b`. */
function shade([r, g, b]: [number, number, number], amount: number): string {
  const target = amount >= 0 ? 255 : 0;
  const a = Math.abs(amount);
  return `${mix(r, target, a)},${mix(g, target, a)},${mix(b, target, a)}`;
}

/** T11.10: a coloured key is a gradient from the colour 12% lighter to the colour 18% darker, with a white icon. */
export function keyFace(color: string | undefined): KeyFace {
  const rgb = color ? parseHex(color) : null;
  if (!rgb) return PLAIN_FACE;
  return {
    background: `linear-gradient(180deg, rgb(${shade(rgb, 0.12)}) 0%, rgb(${shade(rgb, -0.18)}) 100%)`,
    ink: "#ffffff",
    border: "rgba(255,255,255,0.18)",
  };
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
