import { locale, t, type ReminderRepeat } from "@decka/shared";

const DATE_RE = /^(\d{4})-(\d{2})-(\d{2})/;

/**
 * Human label for a recurring reminder (`DATA_MODEL.md 13.4`):
 * `Every day` / `Weekdays` / `Every Mon` (or localized weekday). Returns null for one-time reminders.
 */
export function formatRepeatLabel(
  repeat: ReminderRepeat | undefined,
  at: string,
  loc: string = locale(),
): string | null {
  if (!repeat) return null;
  if (repeat === "daily") return t("rem.repeat_every_day");
  if (repeat === "weekdays") return t("rem.repeat_weekdays");
  const m = DATE_RE.exec(at);
  let day = "";
  if (m) {
    const [, y, mo, d] = m as unknown as [string, string, string, string];
    const date = new Date(Number(y), Number(mo) - 1, Number(d), 12, 0);
    if (!Number.isNaN(date.getTime())) {
      try {
        day = new Intl.DateTimeFormat(loc, { weekday: "short" }).format(date).replace(/\.$/, "");
      } catch {
        day = "";
      }
    }
  }
  return day ? t("rem.repeat_every_weekday", { day }) : t("rem.repeat_weekly");
}
