import type { ListRow, ReminderRow } from "@decka/shared";

/** `2026-11-25T18:30` of a `datetime-local` value (the same text, the browser gives `YYYY-MM-DDTHH:MM`). */
export const isLocalTime = (s: string): boolean => /^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}$/.test(s);

/** The rows of the Reminders tab: finished ones only when `showDone`, soonest first. */
export function visibleReminders(rows: ReminderRow[], showDone: boolean): ReminderRow[] {
  return rows.filter((r) => showDone || !r.done).sort((a, b) => a.at.localeCompare(b.at));
}

/** The list with one item changed, added (no `id`) or removed (`text === null`); the other items keep their ids. */
export function editItems(list: ListRow, change: { id?: string; text?: string | null; done?: boolean }): { id?: string; text: string; done: boolean }[] {
  const items: { id?: string; text: string; done: boolean }[] = list.items.map((i) => ({ id: i.id, text: i.text, done: i.done }));
  if (change.id === undefined) {
    if (change.text) items.push({ text: change.text, done: change.done ?? false });
    return items;
  }
  return items.flatMap((i) => {
    if (i.id !== change.id) return [i];
    if (change.text === null) return [];
    return [{ id: i.id, text: change.text ?? i.text, done: change.done ?? i.done }];
  });
}

/** The default time of a new reminder: an hour from now, rounded up to :00 or :30, as `datetime-local` wants it. */
export function nextHour(now: Date): string {
  const d = new Date(now.getTime() + 3_600_000);
  d.setSeconds(0, 0);
  const m = d.getMinutes();
  if (m !== 0 && m !== 30) d.setMinutes(m < 30 ? 30 : 60);
  const p = (n: number) => String(n).padStart(2, "0");
  return `${d.getFullYear()}-${p(d.getMonth() + 1)}-${p(d.getDate())}T${p(d.getHours())}:${p(d.getMinutes())}`;
}