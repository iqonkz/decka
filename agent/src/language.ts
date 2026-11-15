import type { LanguageSetting } from "@decka/shared";
import { call } from "./ipc";

/** Fired on `window` after the language setting was saved; the root of the window applies it at once. */
export const LANGUAGE_EVENT = "decka:language";

/**
 * Saves `display.language` (only that field of the freshest layout) and tells the window. Returns whether the PC accepted
 * it; the window switches in either case, so the choice is never "stuck" on a failed save.
 */
export async function saveLanguage(language: LanguageSetting): Promise<boolean> {
  window.dispatchEvent(new CustomEvent(LANGUAGE_EVENT, { detail: { language } }));
  try {
    const fresh = await call("get_layout");
    const result = await call("save_layout", { layout: { ...fresh, display: { ...fresh.display, language } } });
    return result.ok;
  } catch {
    return false;
  }
}
