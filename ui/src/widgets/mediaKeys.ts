export type MediaKeyName = "prev" | "play" | "next";

/** The text block of the `media` widget never gets less than this many key sides (DATA_MODEL.md 6.2). */
const MIN_TEXT = 1.5;
const KEY_RATIO = 0.55;
/** Inner padding of the panel, as a share of `side` (DESIGN.md 2). */
const PAD = 0.08;

/**
 * Which transport keys the `media` widget draws (DATA_MODEL.md 6.2).
 * `h = 1`: `prev`/`play`/`next` only from `w >= 4`, otherwise just `play`; `h >= 2`: all three from `w >= 2`.
 * The text block keeps at least `1.5 x side`: when it would get less, the keys go, `prev`/`next` first, `play` last.
 * `side` and `gap` are the key side and the gap of the grid, in px.
 */
export function mediaKeySet(w: number, h: number, side: number, gap: number): MediaKeyName[] {
  const below = h >= 2;
  let keys: MediaKeyName[] = below ? (w >= 2 ? ["prev", "play", "next"] : ["play"]) : w >= 4 ? ["prev", "play", "next"] : ["play"];

  const pad = PAD * side;
  const keySide = Math.round(KEY_RATIO * side);
  const inner = w * side + (w - 1) * gap - 2 * pad;
  const height = h * side + (h - 1) * gap - 2 * pad;
  const row = (n: number) => (n === 0 ? 0 : n * keySide + (n - 1) * gap);
  // Beside the text (h = 1) the keys take width from it; below it (h >= 2) they must fit into its width.
  const textWidth = (n: number) => (below ? inner - height - gap : inner - height - gap - (n === 0 ? 0 : row(n) + gap));
  const fits = (n: number) => textWidth(n) >= MIN_TEXT * side && (!below || row(n) <= textWidth(n));

  while (keys.length > 0 && !fits(keys.length)) keys = keys.length > 1 ? ["play"] : [];
  return keys;
}
