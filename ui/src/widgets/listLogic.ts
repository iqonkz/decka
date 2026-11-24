/** The tick marks the phone guesses before the PC answers (DATA_MODEL.md 13.7): a guess is valid only for the section it was made against. */
export interface Guess {
  source: unknown;
  done: Record<string, boolean>;
}

export const NO_GUESS: Guess = { source: null, done: {} };

/** The guesses that still count: a new `lists` section (a patch) drops them all, which rolls back a toggle the PC refused. */
export function activeGuesses(guess: Guess, source: unknown): Record<string, boolean> {
  return guess.source === source ? guess.done : {};
}

/** The state after a tap: the item is shown with the opposite mark at once. */
export function tapped(guess: Guess, source: unknown, itemId: string, wasDone: boolean): Guess {
  return { source, done: { ...activeGuesses(guess, source), [itemId]: !wasDone } };
}

/** The rows in order: open items first, done ones last, the order inside each group kept. */
export function listRows<T extends { id: string; done: boolean }>(items: T[], guesses: Record<string, boolean>): T[] {
  const marked = items.map((i) => ({ ...i, done: guesses[i.id] ?? i.done }));
  return [...marked.filter((i) => !i.done), ...marked.filter((i) => i.done)];
}