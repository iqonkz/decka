/**
 * T29.P (PROTOCOL.md 3 `page_shown`): the PC is told which page is on the screen, so that "add a button" by voice puts it
 * there. Only the id of the page goes out: after every (re)connection and whenever the page changes.
 */
import { getState, subscribe } from "../store/store";

/** Returns the function that stops following. */
export function installPageShown(send: (pageId: string) => void): () => void {
  let sent: string | null = null;
  return subscribe(() => {
    const s = getState();
    if (s.conn !== "online") {
      sent = null; // the next connection says it again
      return;
    }
    const id = s.state?.layout.pages[s.pageIndex]?.id;
    if (id === undefined || id === sent) return;
    sent = id;
    send(id);
  });
}
