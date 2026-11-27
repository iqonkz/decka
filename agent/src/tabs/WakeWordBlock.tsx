import { useEffect, useRef, useState } from "react";
import { t } from "@decka/shared";
import Button from "../ui/Button";
import Segmented from "../ui/Segmented";
import { Group, Row, SectionHeader } from "../ui/SettingsGroup";
import Switch from "../ui/Switch";
import type { WakeSensitivity, WakeTestPhase, WakeWordConfig } from "./voiceM32Types";

const WAKE_STORAGE_KEY = "decka.wakeword.v1";

function loadStoredWakeWord(): WakeWordConfig {
  try {
    const raw = window.localStorage.getItem(WAKE_STORAGE_KEY);
    if (!raw) return { enabled: false, sensitivity: "medium" };
    const parsed = JSON.parse(raw) as Partial<WakeWordConfig>;
    return {
      enabled: Boolean(parsed.enabled),
      sensitivity:
        parsed.sensitivity === "low" || parsed.sensitivity === "high" ? parsed.sensitivity : "medium",
    };
  } catch {
    return { enabled: false, sensitivity: "medium" };
  }
}

function saveStoredWakeWord(next: WakeWordConfig): void {
  try {
    window.localStorage.setItem(WAKE_STORAGE_KEY, JSON.stringify({ enabled: next.enabled, sensitivity: next.sensitivity }));
  } catch {
    // Storage unavailable (private/SSR test): ignore persistence failure.
  }
}

export interface WakeWordBlockProps {
  initialWakeWord?: WakeWordConfig;
}

/** M32 T32.1b: Wake-word "Decka" toggle, sensitivity picker, and indicator test block. */
export default function WakeWordBlock({ initialWakeWord }: WakeWordBlockProps = {}) {
  const [cfg, setCfg] = useState<WakeWordConfig>(() => initialWakeWord ?? loadStoredWakeWord());
  const [phase, setPhase] = useState<WakeTestPhase>(() => initialWakeWord?.testPhase ?? "idle");
  const timerRef = useRef<number | null>(null);

  useEffect(() => {
    return () => {
      if (timerRef.current !== null) window.clearTimeout(timerRef.current);
    };
  }, []);

  function update(patch: Partial<WakeWordConfig>) {
    const next = { ...cfg, ...patch };
    setCfg(next);
    if (!initialWakeWord) saveStoredWakeWord(next);
  }

  function runTest() {
    if (timerRef.current !== null) window.clearTimeout(timerRef.current);
    setPhase("listening");
    timerRef.current = window.setTimeout(() => {
      timerRef.current = null;
      setPhase("hit");
    }, 900);
  }

  const badgeClass =
    phase === "hit" ? "is-ok" : phase === "listening" ? "is-running" : "is-pending";
  const badgeText =
    phase === "hit"
      ? t("m32.wake_test_hit")
      : phase === "listening"
        ? t("m32.wake_test_listening")
        : t("m32.wake_test_idle");

  return (
    <div id="voice-wake-word" style={{ marginTop: 16 }}>
      <SectionHeader title={t("m32.wake_title")} />
      <Group>
        <Row
          id="section-voice-wake-toggle"
          title={t("m32.wake_title")}
          description={t("m32.wake_desc")}
          icon="waveform"
        >
          <Switch
            checked={cfg.enabled}
            onChange={(enabled) => update({ enabled })}
          />
        </Row>

        <Row id="section-voice-wake-sens" title={t("m32.wake_sensitivity")} icon="sliders-horizontal">
          <Segmented
            options={[
              { value: "low", label: t("m32.wake_low") },
              { value: "medium", label: t("m32.wake_medium") },
              { value: "high", label: t("m32.wake_high") },
            ]}
            value={cfg.sensitivity}
            onChange={(val) => update({ sensitivity: val as WakeSensitivity })}
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
          <Button
            variant={phase === "listening" ? undefined : "primary"}
            onClick={runTest}
          >
            {t("m32.wake_test_btn")}
          </Button>
        </Row>
      </Group>
    </div>
  );
}
