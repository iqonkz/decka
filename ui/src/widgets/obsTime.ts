/** `HH:MM:SS` since `startedAt` (ms since the epoch); the phone counts it, the agent sends only the start (DATA_MODEL.md 6.4). */
export function obsElapsed(startedAt: number, now: number): string {
  const total = Math.max(0, Math.floor((now - startedAt) / 1000));
  const two = (n: number) => String(n).padStart(2, "0");
  return `${two(Math.floor(total / 3600))}:${two(Math.floor((total % 3600) / 60))}:${two(total % 60)}`;
}
