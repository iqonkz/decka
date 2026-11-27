import { t } from "@decka/shared";
import { useDeckEnv } from "../deckEnv";
import WidgetEmpty from "./WidgetEmpty";

export interface CalendarEvent {
  id: string;
  title: string;
  startTime: string;
  endTime?: string;
  joinActionId?: string;
  meetingProvider?: string;
}

export interface CalendarInfo {
  events: CalendarEvent[];
  updatedAt?: string;
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
  const join = (actionId: string) => {
    if (!online) return;
    onPress(actionId).catch((e: { code: string; message?: string }) => onPressError(e));
  };

  return (
    <div className={`calendar calendar-w-${item.w}h-${item.h}`}>
      {/* 1x1 (S): large next event time + concise title */}
      <div className="calendar-s">
        <div className="calendar-s-time widget-figure">{next.startTime}</div>
        <div className="calendar-s-title">{next.title}</div>
      </div>

      {/* 2x1 (M): next meeting row + optional Join action */}
      <div className="calendar-m">
        <div className="calendar-m-info">
          <span className="calendar-time">{next.startTime}</span>
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
            <span className="calendar-time">{ev.startTime}</span>
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
