import { t, type CalendarEvent } from "@decka/shared";
import { useDeckEnv } from "../deckEnv";
import WidgetEmpty from "./WidgetEmpty";

/** When an event is, as the tile says it: the time of the PC, with the date when it is not today. */
export function eventTime(e: CalendarEvent, today: string): string {
  const time = e.allDay ? t("m32.cal_all_day") : e.startTime;
  return e.day === today ? time : `${e.day.slice(8, 10)}.${e.day.slice(5, 7)} ${e.allDay ? "" : time}`.trim();
}

/** `YYYY-MM-DD` of this moment on this device. */
function dayOf(now: Date): string {
  const two = (n: number) => String(n).padStart(2, "0");
  return `${now.getFullYear()}-${two(now.getMonth() + 1)}-${two(now.getDate())}`;
}

/**
 * Upcoming calendar meetings widget (M32 T32.1b): adapts to 1x1 (S), 2x1 (M), and 2x2+ (L)
 * container dimensions; tapping Join sends only `actionId` to the PC.
 */
export default function Calendar({ item }: { item: { id: string; w: number; h: number } }) {
  const { calendar, online, onPress, onPressError } = useDeckEnv();
  const events = calendar?.events ?? [];
  if (events.length === 0) {
    return <WidgetEmpty icon="lucide:calendar" text={t("m32.cal_empty")} />;
  }

  const next = events[0]!;
  const today = dayOf(new Date());
  const join = (actionId: string) => {
    if (!online) return;
    onPress(actionId).catch((e: { code: string; message?: string }) => onPressError(e));
  };

  return (
    <div className={`calendar calendar-w-${item.w}h-${item.h}`}>
      {/* 1x1 (S): large next event time + concise title */}
      <div className="calendar-s">
        <div className="calendar-s-time widget-figure">{eventTime(next, today)}</div>
        <div className="calendar-s-title">{next.title}</div>
      </div>

      {/* 2x1 (M): next meeting row + optional Join action */}
      <div className="calendar-m">
        <div className="calendar-m-info">
          <span className="calendar-time">{eventTime(next, today)}</span>
          <span className="calendar-event-title">{next.title}</span>
        </div>
        {next.joinActionId && (
          <button
            type="button"
            className="calendar-join-btn"
            disabled={!online}
            onClick={() => join(next.joinActionId!)}
          >
            {t("m32.cal_join")}
          </button>
        )}
      </div>

      {/* 2x2+ (L): up to 3 upcoming events with Join buttons */}
      <div className="calendar-l w-l">
        {events.slice(0, 3).map((ev) => (
          <div className="calendar-row" key={ev.id}>
            <span className="calendar-time">{eventTime(ev, today)}</span>
            <span className="calendar-event-title">{ev.title}</span>
            {ev.joinActionId && (
              <button
                type="button"
                className="calendar-join-btn"
                disabled={!online}
                onClick={() => join(ev.joinActionId!)}
              >
                {t("m32.cal_join")}
              </button>
            )}
          </div>
        ))}
      </div>
    </div>
  );
}
