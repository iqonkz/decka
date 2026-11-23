// Phone: what to do when there is no link to the PC (DESIGN.md 4.13). Pure functions: no timers, no DOM.

/** Pauses between background attempts: the first attempt is at once, then these, and the last one repeats. */
export const RETRY_SECONDS = [1, 2, 5, 10];

/** No link for this long: the card with the way out appears (at once when the PC rejected the key). */
export const CARD_AFTER_MS = 8000;

export type LinkConn = "unpaired" | "repair" | "connecting" | "online" | "offline" | "update_required" | "not_your_pc" | "rejected";

/** Why there is no link: the PC does not answer, or it does not know this phone any more. */
export type LinkReason = "silent" | "rejected";

export type LinkView = { card: false } | { card: true; reason: LinkReason };

/** The pause (ms) before background attempt number `attempt` (0 = the first retry after the first failure). */
export function retryDelayMs(attempt: number): number {
  const s = RETRY_SECONDS[Math.min(Math.max(attempt, 0), RETRY_SECONDS.length - 1)] ?? 10;
  return s * 1000;
}

/** A rejected key ends the background attempts: the PC will keep saying no. */
export function keepsRetrying(conn: LinkConn): boolean {
  return conn === "connecting" || conn === "offline";
}

/** What the phone shows after `noLinkMs` without a link. */
export function linkView(conn: LinkConn, noLinkMs: number): LinkView {
  if (conn === "rejected") return { card: true, reason: "rejected" };
  if (keepsRetrying(conn) && noLinkMs >= CARD_AFTER_MS) return { card: true, reason: "silent" };
  return { card: false };
}

/** The buttons of the card: `Connect again` and `Use another PC` always; `Try again` only while the PC may still answer. */
export function cardButtons(reason: LinkReason): { tryAgain: boolean; connectAgain: true; otherPc: true } {
  return { tryAgain: reason === "silent", connectAgain: true, otherPc: true };
}
