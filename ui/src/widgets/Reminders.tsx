import { locale, t, type RemindersItem } from "@decka/shared";
import { useDeckEnv } from "../deckEnv";
import { formatReminderTime } from "./remindersTime";
import WidgetEmpty from "./WidgetEmpty";

/**
 * The next reminders (DATA_MODEL.md 13.7, DESIGN.md 4.16).
 */
export default function Reminders({ item }: { item: RemindersItem }) {
  const { reminders } = useDeckEnv();
  const items = reminders?.items ?? [];
  const now = new Date();
  const rows = item.h * 3 - 1;
  return (
    <div className="rem">
      {items.length === 0 ? (
        <WidgetEmpty icon="lucide:bell" text={t("reminders.none")} />
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
