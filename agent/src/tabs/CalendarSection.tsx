import { useEffect, useState } from "react";
import { t, type CalendarEvent, type CalendarStatus } from "@decka/shared";
import { call } from "../ipc";
import Banner from "../ui/Banner";
import Button from "../ui/Button";
import Field from "../ui/Field";
import PhIcon from "../ui/PhIcon";
import { Group, PageHeader, Row, SectionHeader } from "../ui/SettingsGroup";

/** How often the open page looks for new events. */
const LOOK_EVERY_MS = 3000;

export interface CalendarSectionProps {
  /** Tests and screenshots give the answer of `calendar_get`. */
  initialStatus?: CalendarStatus;
  onSaved?: () => void;
}

/** `02.12 · 10:00 · Zoom` - when an event is and where it meets. */
export function eventWhen(e: CalendarEvent): string {
  const date = `${e.day.slice(8, 10)}.${e.day.slice(5, 7)}`;
  return [date, e.allDay ? t("m32.cal_all_day") : e.startTime, e.meetingProvider].filter(Boolean).join(" · ");
}

/**
 * Settings → Integrations → Calendar (T32.2). The link goes to the agent, which keeps it under DPAPI; the window never gets
 * it back and keeps no copy: what it shows is the masked form the agent gives.
 */
export default function CalendarSection({ initialStatus, onSaved }: CalendarSectionProps = {}) {
  const [status, setStatus] = useState<CalendarStatus | null>(initialStatus ?? null);
  const [url, setUrl] = useState("");
  const [message, setMessage] = useState<{ kind: "success" | "error"; text: string } | null>(null);
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    if (initialStatus) return;
    let live = true;
    const look = () =>
      call("calendar_get").then(
        (s) => {
          if (live) setStatus(s);
        },
        (e) => {
          if (live) setMessage({ kind: "error", text: String(e) });
        },
      );
    void look();
    const id = window.setInterval(() => void look(), LOOK_EVERY_MS);
    return () => {
      live = false;
      window.clearInterval(id);
    };
  }, [initialStatus]);

  function send(link: string) {
    setBusy(true);
    setMessage(null);
    call("calendar_set", { url: link }).then(
      (next) => {
        setBusy(false);
        setStatus(next);
        setUrl(""); // the secret does not stay in the field
        setMessage({ kind: "success", text: t("common.saved") });
        onSaved?.();
      },
      (e) => {
        setBusy(false);
        setMessage({ kind: "error", text: String(e) });
      },
    );
  }

  const refresh = () => call("calendar_refresh").catch((e) => setMessage({ kind: "error", text: String(e) }));
  const events = status?.info?.events ?? [];

  return (
    <div id="section-calendar">
      <PageHeader icon="calendar" title={t("m32.cal_name")} subtitle={t("m32.cal_desc")} />
      {message && <Banner kind={message.kind}>{message.text}</Banner>}
      {status?.info?.error && <Banner kind="error">{status.info.error}</Banner>}

      <Group>
        <div style={{ padding: "12px 16px", display: "flex", flexDirection: "column", gap: 12 }}>
          {status?.masked && (
            <div style={{ fontSize: "var(--font-sm)", color: "var(--muted)", wordBreak: "break-all" }}>
              {t("m32.cal_saved_link")}: <code>{status.masked}</code>
            </div>
          )}
          <Field label={t("m32.cal_url")} hint={t("m32.cal_hint")}>
            <input type="password" autoComplete="off" spellCheck={false} value={url} onChange={(e) => setUrl(e.target.value)} />
          </Field>
          <div style={{ display: "flex", gap: 8, flexWrap: "wrap" }}>
            <Button variant="primary" disabled={busy || url.trim() === ""} onClick={() => send(url.trim())}>
              {t("m32.cal_save")}
            </Button>
            {status?.connected && (
              <>
                <Button disabled={busy} onClick={() => void refresh()}>
                  {t("m32.cal_check")}
                </Button>
                <Button variant="danger" disabled={busy} onClick={() => send("")}>
                  {t("m32.cal_remove")}
                </Button>
              </>
            )}
          </div>
        </div>
      </Group>

      <SectionHeader title={t("m32.cal_upcoming")} />
      <Group>
        {events.length === 0 ? (
          <div style={{ padding: "14px 16px", fontSize: "var(--font-sm)", color: "var(--muted)" }}>{t("m32.cal_empty")}</div>
        ) : (
          events.map((ev) => (
            <Row key={ev.id} title={ev.title} description={eventWhen(ev)} icon="clock">
              {ev.joinActionId && (
                <span style={{ display: "inline-flex", alignItems: "center", gap: 6, fontSize: 12, fontWeight: 600, color: "var(--accent)" }}>
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
