/** `MAJOR.MINOR` of `1.2.3` (also `1.2.3-beta.1`); anything else -> `null`. */
function majorMinor(version: string): string | null {
  const m = /^v?(\d+)\.(\d+)\.\d+(?:[-+][0-9A-Za-z.-]+)?$/.exec(version.trim());
  return m ? `${m[1]}.${m[2]}` : null;
}

/**
 * Whether two versions have the same `MAJOR.MINOR` (ARCHITECTURE.md 8a): phone and PC then work together without a
 * recommendation to update. A version that cannot be read counts as the same (nothing is known, nothing is warned about).
 */
export function sameMinor(a: string | undefined, b: string | undefined): boolean {
  const x = a === undefined ? null : majorMinor(a);
  const y = b === undefined ? null : majorMinor(b);
  return x === null || y === null || x === y;
}
