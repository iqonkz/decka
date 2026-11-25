import { useEffect, useRef, useState } from "react";
import { t } from "@decka/shared";
import Button from "./Button";
import PhIcon from "./PhIcon";

export interface TimeFieldProps {
  value: string;
  onChange: (hhmm: string) => void;
  "aria-label"?: string;
  id?: string;
  disabled?: boolean;
  className?: string;
}

const pad2 = (n: number) => String(n).padStart(2, "0");

export function parseHHMM(v: string): { h: number; m: number } {
  const m = /^(\d{1,2}):(\d{2})$/.exec(v.trim());
  if (!m) return { h: 20, m: 0 };
  return {
    h: Math.max(0, Math.min(23, Number(m[1]))),
    m: Math.max(0, Math.min(59, Number(m[2]))),
  };
}

export function normalizeHHMM(raw: string): string | null {
  const m = /^(\d{1,2}):(\d{2})$/.exec(raw.trim());
  if (!m) return null;
  const h = Number(m[1]);
  const min = Number(m[2]);
  if (h < 0 || h > 23 || min < 0 || min > 59) return null;
  return `${pad2(h)}:${pad2(min)}`;
}

const HOURS = Array.from({ length: 24 }, (_, i) => i);
const MINUTES = [0, 5, 10, 15, 20, 25, 30, 35, 40, 45, 50, 55];

/** Custom 24h HH:mm time field matching DESIGN §4.14/§4.15 (replaces native input[type="time"]). */
export default function TimeField({
  value,
  onChange,
  "aria-label": ariaLabel,
  id,
  disabled = false,
  className,
}: TimeFieldProps) {
  const [open, setOpen] = useState(false);
  const [draft, setDraft] = useState(value);
  const rootRef = useRef<HTMLDivElement>(null);
  const { h, m } = parseHHMM(value);

  useEffect(() => {
    if (!open) setDraft(value);
  }, [value, open]);

  useEffect(() => {
    if (!open) return;
    const onPointerDown = (e: PointerEvent) => {
      if (rootRef.current && !rootRef.current.contains(e.target as Node)) setOpen(false);
    };
    const onKeyDown = (e: KeyboardEvent) => {
      if (e.key === "Escape") setOpen(false);
    };
    window.addEventListener("pointerdown", onPointerDown);
    window.addEventListener("keydown", onKeyDown);
    return () => {
      window.removeEventListener("pointerdown", onPointerDown);
      window.removeEventListener("keydown", onKeyDown);
    };
  }, [open]);

  function commitText(raw: string) {
    setDraft(raw);
    const parsed = normalizeHHMM(raw);
    if (parsed) onChange(parsed);
  }

  return (
    <div ref={rootRef} className={`time-field-wrap${open ? " open" : ""}${className ? ` ${className}` : ""}`}>
      <button
        type="button"
        id={id}
        disabled={disabled}
        aria-label={ariaLabel}
        aria-expanded={open}
        className="time-field-trigger"
        onClick={() => !disabled && setOpen((v) => !v)}
      >
        <PhIcon name="clock" size={14} className="time-field-icon" />
        <span className="time-field-value">{`${pad2(h)}:${pad2(m)}`}</span>
      </button>
      {open && (
        <div className="time-field-popover" role="dialog" aria-label={ariaLabel}>
          <div className="row" style={{ gap: 8 }}>
            <input
              type="text"
              maxLength={5}
              className="time-field-input"
              aria-label={ariaLabel}
              value={draft}
              onChange={(e) => commitText(e.target.value)}
              onBlur={() => setDraft(`${pad2(h)}:${pad2(m)}`)}
              onKeyDown={(e) => {
                if (e.key === "Enter") setOpen(false);
              }}
            />
            <Button variant="primary" onClick={() => setOpen(false)}>{t("common.done")}</Button>
          </div>
          <div className="time-field-cols">
            <div className="time-field-col" role="listbox" aria-label={ariaLabel}>
              {HOURS.map((hh) => (
                <button
                  key={hh}
                  type="button"
                  role="option"
                  aria-selected={hh === h}
                  className={`time-field-cell${hh === h ? " selected" : ""}`}
                  onClick={() => onChange(`${pad2(hh)}:${pad2(m)}`)}
                >
                  {pad2(hh)}
                </button>
              ))}
            </div>
            <div className="time-field-col" role="listbox" aria-label={ariaLabel}>
              {MINUTES.map((mm) => (
                <button
                  key={mm}
                  type="button"
                  role="option"
                  aria-selected={mm === m}
                  className={`time-field-cell${mm === m ? " selected" : ""}`}
                  onClick={() => onChange(`${pad2(h)}:${pad2(mm)}`)}
                >
                  {pad2(mm)}
                </button>
              ))}
            </div>
          </div>
        </div>
      )}
    </div>
  );
}
