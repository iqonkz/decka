/** How many readings the graph keeps: about 2 minutes at the default 2 s interval (DATA_MODEL.md 6.5). */
export const HISTORY_LENGTH = 60;

const round = (n: number): number => Math.round(n * 10) / 10;

/**
 * The line and the area under it for `values` (0..100, oldest first) in a `w` x `h` box. The newest value is at the right
 * edge; a short history starts part-way in. Fewer than two values draw nothing.
 */
export function sparklinePath(values: number[], w: number, h: number): { line: string; area: string } {
  const shown = values.slice(-HISTORY_LENGTH);
  if (shown.length < 2 || w <= 0 || h <= 0) return { line: "", area: "" };
  const step = w / (HISTORY_LENGTH - 1);
  const x0 = w - (shown.length - 1) * step;
  const points = shown.map((v, i) => [round(x0 + i * step), round(h - (Math.min(100, Math.max(0, v)) / 100) * h)] as const);
  const line = points.map(([x, y], i) => `${i === 0 ? "M" : "L"}${x} ${y}`).join(" ");
  const last = points[points.length - 1]!;
  const first = points[0]!;
  return { line, area: `${line} L${last[0]} ${round(h)} L${first[0]} ${round(h)} Z` };
}
