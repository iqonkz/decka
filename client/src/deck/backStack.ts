// The `$back` key (DATA_MODEL.md 3.8): a stack of at most 10 page ids, written by manual moves only.

export const BACK_LIMIT = 10;

/** `from` was left by a manual move (swipe, `navigate` key): it goes on the stack; the oldest falls off past 10. */
export function pushBack(stack: readonly string[], from: string): string[] {
  const next = [...stack, from];
  return next.length > BACK_LIMIT ? next.slice(next.length - BACK_LIMIT) : next;
}

/**
 * The page `$back` opens and the stack without it. Pages that no longer exist (`exists`) are skipped; an empty stack
 * gives `null`: the caller opens the first page.
 */
export function popBack(stack: readonly string[], exists: (pageId: string) => boolean): { pageId: string | null; stack: string[] } {
  const rest = [...stack];
  while (rest.length > 0) {
    const id = rest.pop()!;
    if (exists(id)) return { pageId: id, stack: rest };
  }
  return { pageId: null, stack: [] };
}
