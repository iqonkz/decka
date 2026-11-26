import { useState } from "react";
import { t, tVoice, type TranslationKey } from "@decka/shared";
import { call, listen } from "../ipc";
import Button from "../ui/Button";
import PhIcon from "../ui/PhIcon";
import { Group, Row, SectionHeader } from "../ui/SettingsGroup";
import {
  DEFAULT_VOICE_CHECK_STEPS,
  formatCheckDuration,
  VOICE_CHECK_ORDER,
  voiceErrorCopy,
  type VoiceCheckStep,
  type VoiceCheckStepId,
} from "./voiceM28Types";

export interface VoiceCheckPanelProps {
  /** Deterministic seed for tests and screenshot scenes. */
  initialSteps?: VoiceCheckStep[];
  onRunCheck?: () => Promise<VoiceCheckStep[]> | void;
}

const STEP_TITLE: Record<VoiceCheckStepId, TranslationKey> = {
  mic: "m28.check_step_mic",
  stt: "m28.check_step_stt",
  model: "m28.check_step_model",
  action: "m28.check_step_action",
};

const STEP_DESC: Record<VoiceCheckStepId, TranslationKey> = {
  mic: "m28.check_step_mic_desc",
  stt: "m28.check_step_stt_desc",
  model: "m28.check_step_model_desc",
  action: "m28.check_step_action_desc",
};

function stepIcon(state: VoiceCheckStep["state"]) {
  switch (state) {
    case "ok":
      return <PhIcon name="check-circle" size={18} color="var(--ok)" />;
    case "error":
      return <PhIcon name="x-circle" size={18} color="var(--danger)" />;
    case "running":
      return <PhIcon name="circle-notch" size={18} color="var(--accent)" className="voice-check-spin" />;
    case "pending":
    default:
      return <PhIcon name="circle" size={18} color="var(--muted)" />;
  }
}

function stepStatusLabel(step: VoiceCheckStep): string {
  const time = formatCheckDuration(step.durationMs);
  if (step.state === "ok") return t("m28.check_status_ok", { time: time || "0 ms" });
  if (step.state === "error") {
    return time ? t("m28.check_status_error", { time }) : t("m28.check_status_error_notime");
  }
  if (step.state === "running") return t("m28.check_status_running");
  return t("m28.check_status_pending");
}

/** The line under a step: when to speak, why it failed, or what it found; before it ran, what it will do. */
export function stepLine(step: VoiceCheckStep): string {
  if (step.listening) return t("voice.check.speak");
  if (step.state === "error") {
    if (step.reason) return voiceErrorCopy(step.reason).reason;
    return tVoice(step.errorKey, undefined, step.error) || step.error || t(STEP_DESC[step.id]);
  }
  if (step.understood) {
    return t("voice.stage.understood", { what: tVoice(step.understood.key, step.understood.args, step.understood.text) || step.understood.text });
  }
  if (step.id === "stt" && step.state === "ok" && step.detail) return t("voice.stage.heard", { text: step.detail });
  return step.detail ?? t(STEP_DESC[step.id]);
}

/**
 * 4-step "Check voice" diagnostic panel in Settings -> Voice (TASKS.md §T28.3, §T28.2b).
 */
export default function VoiceCheckPanel({ initialSteps, onRunCheck }: VoiceCheckPanelProps) {
  const [steps, setSteps] = useState<VoiceCheckStep[]>(initialSteps ?? DEFAULT_VOICE_CHECK_STEPS);
  const [running, setRunning] = useState(false);

  const byId = new Map(steps.map((s) => [s.id, s]));
  const hasCompleted = steps.some((s) => s.state === "ok" || s.state === "error");

  const runCheck = async () => {
    setRunning(true);
    setSteps(VOICE_CHECK_ORDER.map((id, idx) => ({ id, state: idx === 0 ? "running" : "pending" })));
    let unlisten: (() => void) | undefined;
    try {
      // Every change of the steps arrives while the check runs (among them "speak now").
      if (!onRunCheck) unlisten = await listen<VoiceCheckStep[]>("voice-check", (e) => setSteps(e.payload));
      const res = onRunCheck ? await onRunCheck() : await call("voice_self_check");
      if (Array.isArray(res) && res.length > 0) {
        setSteps(res);
      }
    } catch (err) {
      const msg = String(err);
      setSteps(
        VOICE_CHECK_ORDER.map((id, idx) =>
          idx === 0 ? { id, state: "error", error: msg } : { id, state: "pending" },
        ),
      );
    } finally {
      unlisten?.();
      setRunning(false);
    }
  };

  return (
    <div id="voice-check" style={{ marginTop: 24 }}>
      <SectionHeader id="section-voice-check" title={t("m28.check_title")} subtitle={t("m28.check_subtitle")} />
      <Group>
        <Row title={t("m28.check_title")} description={t("m28.check_subtitle")}>
          <Button variant="primary" disabled={running} onClick={() => void runCheck()}>
            {running ? t("m28.check_running") : hasCompleted ? t("m28.check_rerun") : t("m28.check_run")}
          </Button>
        </Row>
        {VOICE_CHECK_ORDER.map((id) => {
          const step = byId.get(id) ?? { id, state: "pending" as const };
          const desc = stepLine(step);
          return (
            <Row
              key={id}
              title={
                <span className="voice-check-step-title">
                  {stepIcon(step.state)}
                  <span>{t(STEP_TITLE[id])}</span>
                </span>
              }
              description={desc}
            >
              <span className={`voice-check-status is-${step.state}`}>{stepStatusLabel(step)}</span>
            </Row>
          );
        })}
      </Group>
    </div>
  );
}
