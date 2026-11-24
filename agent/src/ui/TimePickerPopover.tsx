import { useEffect, useRef, useState } from "react";
import { locale, t } from "@decka/shared";
import PhIcon from "./PhIcon";
import Button from "./Button";

export function pad2(n: number): string {
  return String(n).padStart(2, "0");
}

export function toLocalIso(d: Date): string {
  return `${d.getFullYear()}-${pad2(d.getMonth() + 1)}-${pad2(d.getDate())}T${pad2(d.getHours())}:${pad2(d.getMinutes())}`;
}

export function parseLocalIso(iso: string): Date {
  const parts = /^(\d{4})-(\d{2})-(\d{2})T(\d{2}):(\d{2})/.exec(iso);
  if (!parts) return new Date();
  return new Date(
    Number(parts[1]),
    Number(parts[2]) - 1,
    Number(parts[3]),
    Number(parts[4]),
    Number(parts[5]),
  );
}

export function isOverdue(iso: string): boolean {
  const d = parseLocalIso(iso);
  return d.getTime() < Date.now();
}

export function formatReminderLabel(iso: string): string {
  const d = parseLocalIso(iso);
  const now = new Date();
  const timeStr = `${pad2(d.getHours())}:${pad2(d.getMinutes())}`;

  const isToday =
    d.getFullYear() === now.getFullYear() &&
    d.getMonth() === now.getMonth() &&
    d.getDate() === now.getDate();

  if (isToday) {
    return t("rem.today_at", { time: timeStr });
  }

  const tomorrow = new Date(now.getFullYear(), now.getMonth(), now.getDate() + 1);
  const isTomorrow =
    d.getFullYear() === tomorrow.getFullYear() &&
    d.getMonth() === tomorrow.getMonth() &&
    d.getDate() === tomorrow.getDate();

  if (isTomorrow) {
    return t("rem.tomorrow_at", { time: timeStr });
  }

  try {
    return new Intl.DateTimeFormat(locale(), {
      day: "numeric",
      month: "short",
      hour: "2-digit",
      minute: "2-digit",
    }).format(d);
  } catch {
    return `${pad2(d.getDate())}.${pad2(d.getMonth() + 1)} ${timeStr}`;
  }
}

export interface TimePickerPopoverProps {
  value: string;
  onChange: (iso: string) => void;
  onClose: () => void;
}

export default function TimePickerPopover({ value, onChange, onClose }: TimePickerPopoverProps) {
  const popoverRef = useRef<HTMLDivElement>(null);
  const initialDate = parseLocalIso(value);
  const [viewYear, setViewYear] = useState(initialDate.getFullYear());
  const [viewMonth, setViewMonth] = useState(initialDate.getMonth()); // 0..11

  const currentD = parseLocalIso(value);
  const timeStr = `${pad2(currentD.getHours())}:${pad2(currentD.getMinutes())}`;

  useEffect(() => {
    const onPointerDown = (e: PointerEvent) => {
      if (popoverRef.current && !popoverRef.current.contains(e.target as Node)) {
        onClose();
      }
    };
    const onKeyDown = (e: KeyboardEvent) => {
      if (e.key === "Escape") onClose();
    };
    window.addEventListener("pointerdown", onPointerDown);
    window.addEventListener("keydown", onKeyDown);
    return () => {
      window.removeEventListener("pointerdown", onPointerDown);
      window.removeEventListener("keydown", onKeyDown);
    };
  }, [onClose]);

  const setHourQuick = () => {
    const next = new Date(Date.now() + 60 * 60 * 1000);
    onChange(toLocalIso(next));
    onClose();
  };

  const setTomorrowQuick = () => {
    const now = new Date();
    const tom = new Date(now.getFullYear(), now.getMonth(), now.getDate() + 1, 9, 0, 0);
    onChange(toLocalIso(tom));
    onClose();
  };

  const prevMonth = () => {
    if (viewMonth === 0) {
      setViewYear(viewYear - 1);
      setViewMonth(11);
    } else {
      setViewMonth(viewMonth - 1);
    }
  };

  const nextMonth = () => {
    if (viewMonth === 11) {
      setViewYear(viewYear + 1);
      setViewMonth(0);
    } else {
      setViewMonth(viewMonth + 1);
    }
  };

  const selectDay = (day: number) => {
    const next = new Date(viewYear, viewMonth, day, currentD.getHours(), currentD.getMinutes());
    onChange(toLocalIso(next));
  };

  const handleTimeChange = (newTime: string) => {
    const parts = /^(\d{2}):(\d{2})/.exec(newTime);
    if (!parts) return;
    const next = new Date(
      currentD.getFullYear(),
      currentD.getMonth(),
      currentD.getDate(),
      Number(parts[1]),
      Number(parts[2]),
    );
    onChange(toLocalIso(next));
  };

  // Calendar calculations
  const firstDayOfWeek = (new Date(viewYear, viewMonth, 1).getDay() + 6) % 7; // Monday = 0
  const daysInMonth = new Date(viewYear, viewMonth + 1, 0).getDate();

  const monthTitle = new Date(viewYear, viewMonth, 1).toLocaleString(locale(), {
    month: "long",
    year: "numeric",
  });

  return (
    <div className="rem-popover" ref={popoverRef} role="dialog">
      {/* Quick buttons */}
      <div className="rem-popover-quick">
        <Button variant="secondary" onClick={setHourQuick}>
          {t("rem.in_one_hour")}
        </Button>
        <Button variant="secondary" onClick={setTomorrowQuick}>
          {t("rem.tomorrow_morning")}
        </Button>
      </div>

      {/* Month Calendar Grid */}
      <div className="rem-calendar">
        <div className="rem-calendar-nav">
          <button type="button" className="rem-cal-nav-btn" onClick={prevMonth} aria-label={t("rem.prev_month")}>
            <PhIcon name="caret-left" size={14} />
          </button>
          <span className="rem-cal-month-title">{monthTitle}</span>
          <button type="button" className="rem-cal-nav-btn" onClick={nextMonth} aria-label={t("rem.next_month")}>
            <PhIcon name="caret-right" size={14} />
          </button>
        </div>

        <div className="rem-calendar-weekdays">
          {["Mo", "Tu", "We", "Th", "Fr", "Sa", "Su"].map((d) => (
            <span key={d} className="rem-cal-weekday">
              {d}
            </span>
          ))}
        </div>

        <div className="rem-calendar-days">
          {Array.from({ length: firstDayOfWeek }).map((_, i) => (
            <span key={`empty-${i}`} className="rem-cal-day empty" />
          ))}
          {Array.from({ length: daysInMonth }).map((_, i) => {
            const dayNum = i + 1;
            const isSelected =
              currentD.getFullYear() === viewYear &&
              currentD.getMonth() === viewMonth &&
              currentD.getDate() === dayNum;
            return (
              <button
                key={dayNum}
                type="button"
                className={`rem-cal-day ${isSelected ? "selected" : ""}`}
                onClick={() => selectDay(dayNum)}
              >
                {dayNum}
              </button>
            );
          })}
        </div>
      </div>

      {/* Time & Done */}
      <div className="rem-popover-time-row">
        <div className="rem-popover-time-field">
          <PhIcon name="clock" size={16} />
          <input
            type="time"
            className="rem-time-input"
            value={timeStr}
            onChange={(e) => handleTimeChange(e.target.value)}
          />
        </div>
        <Button variant="primary" onClick={onClose}>
          {t("common.done")}
        </Button>
      </div>
    </div>
  );
}
