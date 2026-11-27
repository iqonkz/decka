import { useEffect, useState } from "react";
import { t, type WakeSensitivity, type WakeStatus } from "@decka/shared";
import { call, listen } from "../ipc";
import Banner from "../ui/Banner";
import Button from "../ui/Button";
import Segmented from "../ui/Segmented";
import { Group, Row, SectionHeader } from "../ui/SettingsGroup";
import Switch from "../ui/Switch";

export interface WakeWordBlockProps {
  /** Tests and screenshots give the answer of `wake_get`. */
  initialStatus?: WakeStatus;
  /** `heard` when "Say Decka" was pressed (tests). */
  initialTestFrom?: number;
}

/** What the line under the switch says besides the privacy notice; null when there is nothing to add. */
export function wakeStateText(status: WakeStatus): string | null {
  if (!status.enabled) return status.installed ? null : t("m32.wake_download_note", { mb: Math.round(status.size / 1_000_000) });
  switch (status.state) {
    case "downloading":
      return t("m32.wake_downloading", { pct: status.percent ?? 0 });
    case "paused":
      return t("m32.wake_paused");
    case "off":
      return t("m32.wake_needs_voice");
    case "error":
      return status.reason;
    default:
      return null;
  }
}

/** Settings → Voice: the wake word "Decka" - the switch, the sensitivity and "Say Decka" (T32.1). */
export default function WakeWordBlock({ initialStatus, initialTestFrom }: WakeWordBlockProps = {}) {
  const [status, setStatus] = useState<WakeStatus | null>(initialStatus ?? null);
  /** The count of detections when the test began; null = no test was made. */
  const [testFrom, setTestFrom] = useState<number | null>(initialTestFrom ?? null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (initialStatus) return;
    let live = true;
    let off: (() => void) | undefined;
    const failed = (e: unknown) => {
      if (live) setError(String(e));
    };
    call("wake_get").then((s) => {
      if (live) setStatus(s);
    }, failed);
    listen<WakeStatus>("wake-status", (e) => setStatus(e.payload)).then((fn) => {
      if (live) off = fn;
      else fn();
    }, failed);
    return () => {
      live = false;
      off?.();
    };
  }, [initialStatus]);

  function apply(enabled: boolean, sensitivity: WakeSensitivity) {
    setError(null);
    call("wake_set", { enabled, sensitivity }).then(setStatus, (e) => setError(String(e)));
  }

  function runTest() {
    if (!status) return;
    setError(null);
    setTestFrom(status.heard);
    call("wake_test", { on: true }).then(setStatus, (e) => setError(String(e)));
  }

  const enabled = status?.enabled ?? false;
  const sensitivity = status?.sensitivity ?? "medium";
  const hit = status !== null && testFrom !== null && status.heard > testFrom;
  const listening = !hit && (status?.testing ?? false);
  const badgeClass = hit ? "is-ok" : listening ? "is-running" : "is-pending";
  const badgeText = hit ? t("m32.wake_test_hit") : listening ? t("m32.wake_test_listening") : t("m32.wake_test_idle");
  const extra = status ? wakeStateText(status) : null;

  return (
    <div id="voice-wake-word" style={{ marginTop: 16 }}>
      <SectionHeader title={t("m32.wake_title")} />
      {error && <Banner kind="error">{error}</Banner>}
      <Group>
        <Row
          id="section-voice-wake-toggle"
          title={t("m32.wake_title")}
          description={
            <>
              {t("m32.wake_desc")}
              {extra && <span style={{ display: "block", marginTop: 4 }}>{extra}</span>}
            </>
          }
          icon="waveform"
        >
          <Switch checked={enabled} disabled={status === null} onChange={(on) => apply(on, sensitivity)} />
        </Row>

        <Row id="section-voice-wake-sens" title={t("m32.wake_sensitivity")} icon="sliders-horizontal">
          <Segmented
            options={[
              { value: "low", label: t("m32.wake_low") },
              { value: "medium", label: t("m32.wake_medium") },
              { value: "high", label: t("m32.wake_high") },
            ]}
            value={sensitivity}
            onChange={(val) => apply(enabled, val as WakeSensitivity)}
          />
        </Row>

        <Row
          id="section-voice-wake-test"
          title={t("m32.wake_test_prompt")}
          description={
            <span className={`voice-check-status ${badgeClass}`} style={{ marginTop: 4, display: "inline-block" }}>
              {badgeText}
            </span>
          }
          icon="microphone"
        >
          <Button variant={listening ? undefined : "primary"} disabled={status?.state !== "listening"} onClick={runTest}>
            {t("m32.wake_test_btn")}
          </Button>
        </Row>
      </Group>
    </div>
  );
}
