/** `list` with the item at `from` moved to index `to` (both clamped); the same array when nothing moves. */
export function moveStep<T>(list: readonly T[], from: number, to: number): readonly T[] {
  if (from < 0 || from >= list.length) return list;
  const target = Math.min(list.length - 1, Math.max(0, to));
  if (target === from) return list;
  const copy = [...list];
  const [item] = copy.splice(from, 1);
  copy.splice(target, 0, item as T);
  return copy;
}

/** The index a dragged step lands on: the first row whose middle is below the pointer (`mids` are the rows' vertical middles). */
export function dropIndex(mids: readonly number[], pointerY: number): number {
  const i = mids.findIndex((m) => pointerY < m);
  return i < 0 ? Math.max(0, mids.length - 1) : i;
}
