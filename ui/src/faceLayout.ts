/** How a key with the `full` face draws its icon (DESIGN.md 2.1). */
export interface FaceLayout {
  /** `cover`: the picture fills the key; `contain`: a picture of a program, centred; `center`: a vector sign, centred; `none`: no icon. */
  mode: "cover" | "contain" | "center" | "none";
  /** Size of the picture or sign in px (the whole key for `cover`). */
  iconPx: number;
  /** The label lies over the picture on a dark gradient (otherwise there is no label to draw). */
  overlayLabel: boolean;
}

/** Share of the key a vector sign (lucide, emoji, Phosphor) takes in `full`. */
export const VECTOR_SHARE = 0.68;
/** Share of the key a transparent program picture takes in `full`. */
export const CONTAIN_SHARE = 0.84;

/**
 * The table of DESIGN.md 2.1: an opaque square picture (`isTile`) covers the key; a transparent picture is `contain`ed at
 * 0.84 x side; lucide, emoji and an SVG are centred at 0.68 x side. `url` (the data URL of an `img:` icon) tells SVG from PNG.
 */
export function faceLayout(icon: string | undefined, isTile: boolean, side: number, showLabel: boolean, url?: string): FaceLayout {
  if (!icon) return { mode: "none", iconPx: 0, overlayLabel: false };
  const vector = !icon.startsWith("img:") || (url !== undefined && url.startsWith("data:image/svg+xml"));
  if (vector) return { mode: "center", iconPx: Math.round(VECTOR_SHARE * side), overlayLabel: showLabel };
  if (isTile) return { mode: "cover", iconPx: Math.round(side), overlayLabel: showLabel };
  return { mode: "contain", iconPx: Math.round(CONTAIN_SHARE * side), overlayLabel: showLabel };
}

/** The face a button has: its own `face`, else `display.keyFace`, else `standard`. */
export function resolveFace(own: "standard" | "full" | undefined, display: "standard" | "full" | undefined): "standard" | "full" {
  return own ?? display ?? "standard";
}

/** Whether the label of a button shows: it is not empty and (`showLabel` ?? `display.keyLabels` ?? true). */
export function labelShown(label: string | undefined, own: boolean | undefined, display: boolean | undefined): boolean {
  return label !== undefined && label !== "" && (own ?? display ?? true);
}

/** Alpha (0..255) of the four corner pixels at or above which a picture counts as an opaque tile. */
export const TILE_ALPHA = 250;

/** `true` when all four corners are opaque enough: the picture is a square tile, not a transparent program icon. */
export function isTileCorners(alphas: [number, number, number, number]): boolean {
  return alphas.every((a) => a >= TILE_ALPHA);
}

/** Opacity of the dark layer at the very bottom of the label gradient (DESIGN.md 2.1). */
export const LABEL_GRADIENT_ALPHA = 0.72;