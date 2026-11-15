/** "<n> Kbps" below 1000, otherwise "<n.n> Mbps". */
export function formatRate(kbps: number): string {
  if (kbps < 1000) return `${Math.round(kbps)} Kbps`;
  return `${(kbps / 1000).toFixed(1)} Mbps`;
}

/** Integer percentage; 0 when `total` is 0. */
export function percent(used: number, total: number): number {
  return total === 0 ? 0 : Math.round((used / total) * 100);
}

const two = (n: number): string => String(n).padStart(2, "0");

/** 24-hour clock: HH:MM, or HH:MM:SS with `seconds`. */
export function formatClock(d: Date, seconds: boolean): string {
  const base = `${two(d.getHours())}:${two(d.getMinutes())}`;
  return seconds ? `${base}:${two(d.getSeconds())}` : base;
}

/** Short date in the device locale, e.g. "Sat, 14 Nov". */
export function formatDate(d: Date): string {
  return d.toLocaleDateString(undefined, { weekday: "short", day: "numeric", month: "short" });
}
