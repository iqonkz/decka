/**
 * "Go to page X" by voice (DATA_MODEL.md 12.5 `go_page`): the PC resolves the page and publishes the variable `voice.goto`
 * (`<pageId>#<counter>`); the phone only follows it. It never sends a page back.
 */
import { getState, navigateManually, subscribe } from "../store/store";
import { pageIndexOf, parseGoto } from "./gotoMath";

/**
 * Follows `voice.goto`: the first value seen after a (re)connect is only remembered, a later change navigates.
 * Returns the function that stops following.
 */
export function installVoiceGoto(): () => void {
  let seen: string | null | undefined;
  return subscribe(() => {
    const state = getState().state;
    if (!state) {
      seen = undefined;
      return;
    }
    const value = state.vars?.["voice.goto"];
    const current = typeof value === "string" ? value : null;
    if (seen === undefined) {
      seen = current;
      return;
    }
    if (current === seen) return;
    seen = current;
    const id = parseGoto(current);
    if (id === null) return;
    const index = pageIndexOf(state.layout.pages, id);
    if (index >= 0) navigateManually(index);
  });
}
