import { locale, t, type RemindersItem } from "@decka/shared";
import { useDeckEnv } from "../deckEnv";
import { formatReminderTime } from "./remindersTime";

/**
 * The next reminders (DATA_MODEL.md 13.7): the time on the left (`HH:mm`, or the weekday for another day), the text on the right,
 * one line each. The PC sends at most 5; the widget shows as many as fit its height.
 */
export default function Reminders({ item }: { item: RemindersItem }) {
  const { reminders } = useDeckEnv();
  const items = reminders?.items ?? [];
  const now = new Date();
  const rows = Math.max(1, item.h * 2 - 1);
  return (
    <div className="rem">
      <div className="rem-title">{t("reminders.title")}</div>
      {items.length === 0 ? (
        <div className="rem-none">{t("reminders.none")}</div>
      ) : (
        items.slice(0, rows).map((r) => (
          <div className="rem-row" key={r.id}>
            <span className="rem-time">{formatReminderTime(r.at, now, locale())}</span>
            <span className="rem-text">{r.text}</span>
          </div>
        ))
      )}
    </div>
  );
}