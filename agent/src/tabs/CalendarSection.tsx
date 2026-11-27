import { useState } from "react";
import { t } from "@decka/shared";
import Banner from "../ui/Banner";
import Button from "../ui/Button";
import Field from "../ui/Field";
import PhIcon from "../ui/PhIcon";
import { Group, PageHeader, Row, SectionHeader } from "../ui/SettingsGroup";
import type { CalendarConfig } from "./voiceM32Types";

const CAL_STORAGE_KEY = "decka.calendar.v1";

export function loadStoredCalendar(): CalendarConfig {
  try {
    const raw = window.localStorage.getItem(CAL_STORAGE_KEY);
    if (!raw) return { url: "", connected: false, events: [] };
    const parsed = JSON.parse(raw) as Partial<CalendarConfig>;
    return {
      url: parsed.url ?? "",
      connected: Boolean(parsed.url),
      events: Array.isArray(parsed.events) ? parsed.events : [],
    };
  } catch {
    return { url: "", connected: false, events: [] };
  }
}

function saveStoredCalendar(next: CalendarConfig): void {
  try {
    window.localStorage.setItem(CAL_STORAGE_KEY, JSON.stringify(next));
  } catch {
    // Storage blocked in SSR tests.
  }
}

export interface CalendarSectionProps {
  initialStatus?: CalendarConfig;
  onSaved?: () => void;
}

/** M32 T32.1b: Calendar (ICS) settings section — feed URL, DPAPI/refresh hint, and upcoming events. */
export default function CalendarSection({ initialStatus, onSaved }: CalendarSectionProps = {}) {
  const [status, setStatus] = useState<CalendarConfig>(() => initialStatus ?? loadStoredCalendar());
  const [url, setUrl] = useState(() => status.url);
  const [message, setMessage] = useState<{ kind: "success" | "error"; text: string } | null>(null);

  function saveUrl() {
    const clean = url.trim();
    const next: CalendarConfig = {
      url: clean,
      connected: clean.length > 0,
      events: status.events,
    };
    setStatus(next);
    if (!initialStatus) saveStoredCalendar(next);
    setMessage({ kind: "success", text: t("common.saved") });
    onSaved?.();
  }

  function disconnect() {
    const next: CalendarConfig = { url: "", connected: false, events: [] };
    setUrl("");
    setStatus(next);
    if (!initialStatus) saveStoredCalendar(next);
    setMessage(null);
    onSaved?.();
  }

  return (
    <div id="section-calendar">
      <PageHeader icon="calendar" title={t("m32.cal_name")} subtitle={t("m32.cal_desc")} />
      {message && <Banner kind={message.kind}>{message.text}</Banner>}

      <Group>
        <div style={{ padding: "12px 16px", display: "flex", flexDirection: "column", gap: 12 }}>
          <Field label={t("m32.cal_url")} hint={t("m32.cal_hint")}>
            <input
              type="url"
              value={url}
              onChange={(e) => setUrl(e.target.value)}
            />
          </Field>
          <div style={{ display: "flex", gap: 8, flexWrap: "wrap" }}>
            <Button variant="primary" disabled={url.trim() === ""} onClick={saveUrl}>
              {t("m32.cal_save")}
            </Button>
            {status.connected && (
              <>
                <Button onClick={saveUrl}>{t("m32.cal_check")}</Button>
                <Button variant="danger" onClick={disconnect}>
                  {t("m32.cal_remove")}
                </Button>
              </>
            )}
          </div>
        </div>
      </Group>

      <SectionHeader title={t("m32.cal_upcoming")} />
      <Group>
        {status.events.length === 0 ? (
          <div style={{ padding: "14px 16px", fontSize: "var(--font-sm)", color: "var(--muted)" }}>
            {t("m32.cal_empty")}
          </div>
        ) : (
          status.events.map((ev) => (
            <Row
              key={ev.id}
              title={ev.title}
              description={ev.meetingProvider ? `${ev.startTime} · ${ev.meetingProvider}` : ev.startTime}
              icon="clock"
            >
              {ev.joinUrl && (
                <span
                  style={{
                    display: "inline-flex",
                    alignItems: "center",
                    gap: 6,
                    fontSize: 12,
                    fontWeight: 600,
                    color: "var(--accent)",
                  }}
                >
                  <PhIcon name="video-camera" size={15} />
                  {t("m32.cal_join")}
                </span>
              )}
            </Row>
          ))
        )}
      </Group>
    </div>
  );
}
