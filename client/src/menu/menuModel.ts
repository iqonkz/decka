import { t } from "@decka/shared";
import type { ConnState } from "../store/store";

/** How long a finger must stay on an empty spot to open the menu (DESIGN.md 4.3). */
export const MENU_HOLD_MS = 1200;
/** A finger that moves further than this while holding is a swipe, not a hold. */
export const MENU_HOLD_SLOP_PX = 14;

/** `Connected to DESKTOP · USB`, `Connected to DESKTOP · Wi-Fi` or `Offline`. */
export function statusLine(conn: ConnState, via: "usb" | "lan" | null, agentName: string | null): string {
  if (conn !== "online") return t("menu.offline");
  const route = via === "usb" ? " · USB" : via === "lan" ? " · Wi-Fi" : "";
  return `${t("menu.connected", { pc: agentName ?? t("menu.your_pc") })}${route}`;
}

/** `App 0.1.0 · PC 0.1.0`; without a connection (the PC version is not known yet) only `App 0.1.0`. */
export function versionLine(app: string, pc: string | null): string {
  return pc === null ? t("menu.app_version", { app }) : `${t("menu.app_version", { app })} · ${t("menu.pc_version_short", { pc })}`;
}

/**
 * A touch opens the menu only on empty space: not on a key, a widget, an open group, a toast or a dialog.
 * `closest` is the element's `closest` function result for the selector of interactive parts.
 */
export const INTERACTIVE_SELECTOR = "[data-tile], .key, .tile, .voice-fab, .popup-backdrop, .toasts, .dialog-backdrop, .menu-sheet, .menu-backdrop";

export function isEmptySpot(target: EventTarget | null): boolean {
  return typeof Element !== "undefined" && target instanceof Element && target.closest(INTERACTIVE_SELECTOR) === null;
}
