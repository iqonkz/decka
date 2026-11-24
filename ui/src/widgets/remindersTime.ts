/** `at` is a local time `YYYY-MM-DDTHH:MM` (DATA_MODEL.md 13.7); `now` is the phone's clock. */
const AT_RE = /^(\d{4})-(\d{2})-(\d{2})T(\d{2}):(\d{2})$/;

/**
 * The time column of the `reminders` widget: `HH:mm` for today, `Tue 09:00` (short weekday in the interface language) for any
 * other day. A malformed value is shown as it came.
 */
export function formatReminderTime(at: string, now: Date, locale: string): string {
  const m = AT_RE.exec(at);
  if (!m) return at;
  const [, y, mo, d, h, mi] = m as unknown as [string, string, string, string, string, string];
  const clock = `${h}:${mi}`;
  const day = new Date(Number(y), Number(mo) - 1, Number(d), 12, 0);
  const same = day.getFullYear() === now.getFullYear() && day.getMonth() === now.getMonth() && day.getDate() === now.getDate();
  if (same) return clock;
  let weekday: string;
  try {
    weekday = new Intl.DateTimeFormat(locale, { weekday: "short" }).format(day);
  } catch {
    weekday = "";
  }
  return `${weekday} ${clock}`.trim();
}