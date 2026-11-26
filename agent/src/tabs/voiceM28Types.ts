import { t, type VoiceCheckStep, type VoiceErrorReason } from "@decka/shared";

/** The words of the voice failure reasons and of `Check voice` (TASKS.md §T28.2, §T28.3, §T28.2b). */
export type { VoiceCheckStep, VoiceErrorReason };
export type VoiceCheckStepId = VoiceCheckStep["id"];

export const VOICE_CHECK_ORDER: VoiceCheckStepId[] = ["mic", "stt", "model", "action"];

export const DEFAULT_VOICE_CHECK_STEPS: VoiceCheckStep[] = VOICE_CHECK_ORDER.map((id) => ({
  id,
  state: "pending",
}));

export interface VoiceErrorCopy {
  short: string;
  reason: string;
  hint: string;
  buttonLabel: string;
  action: "redownload" | "switch_light" | "retry" | "check_mic";
}

/** Maps a structured voice failure reason to localized Island & Settings copy (T28.2). */
export function voiceErrorCopy(reason: VoiceErrorReason, exitCode?: number): VoiceErrorCopy {
  switch (reason) {
    case "vram":
      return {
        short: t("m28.err_vram_short"),
        reason: t("m28.err_vram_reason"),
        hint: t("m28.err_vram_hint"),
        buttonLabel: t("m28.err_btn_switch_light"),
        action: "switch_light",
      };
    case "ram":
      return {
        short: t("m28.err_ram_short"),
        reason: t("m28.err_ram_reason"),
        hint: t("m28.err_ram_hint"),
        buttonLabel: t("m28.err_btn_switch_light"),
        action: "switch_light",
      };
    case "model_file":
      return {
        short: t("m28.err_model_file_short"),
        reason: t("m28.err_model_file_reason"),
        hint: t("m28.err_model_file_hint"),
        buttonLabel: t("m28.err_btn_redownload"),
        action: "redownload",
      };
    case "port":
      return {
        short: t("m28.err_port_short"),
        reason: t("m28.err_port_reason"),
        hint: t("m28.err_port_hint"),
        buttonLabel: t("m28.err_btn_retry"),
        action: "retry",
      };
    case "mic":
      return {
        short: t("m28.err_mic_short"),
        reason: t("m28.err_mic_reason"),
        hint: t("m28.err_mic_hint"),
        buttonLabel: t("m28.err_btn_check_mic"),
        action: "check_mic",
      };
    case "stt":
      return {
        short: t("m28.err_stt_short"),
        reason: t("m28.err_stt_reason"),
        hint: t("m28.err_stt_hint"),
        buttonLabel: t("m28.err_btn_retry"),
        action: "retry",
      };
    case "crash":
    default:
      return {
        short: t("m28.err_crash_short"),
        reason: t("m28.err_crash_reason", { code: exitCode ?? -1 }),
        hint: t("m28.err_crash_hint"),
        buttonLabel: t("m28.err_btn_retry"),
        action: "retry",
      };
  }
}

export function formatCheckDuration(ms?: number): string {
  if (ms === undefined || !Number.isFinite(ms)) return "";
  if (ms >= 1000) return `${(ms / 1000).toFixed(1)} s`;
  return `${Math.max(1, Math.round(ms))} ms`;
}
