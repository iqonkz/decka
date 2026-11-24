import { useEffect, useState } from "react";
import { t, tVoice } from "@decka/shared";
import logoUrl from "../../docs/assets/decka-logo.svg";
import { call, onVoicePill, type VoicePillEvent } from "./ipc";

export interface VoicePillProps {
  initialData?: VoicePillEvent;
  onConfirm?: (yes: boolean) => void;
}

const DEFAULT_EVENT: VoicePillEvent = {
  state: "listening",
  level: 0,
};

/** Heights of the 5 bars of the wave: the middle one is the tallest, a quiet room leaves them lying at 4 px (DESIGN.md 4.12). */
export const WAVE_COEFFICIENTS = [0.6, 0.85, 1, 0.85, 0.6];
export const WAVE_MAX_PX = 24;
export const WAVE_MIN_PX = 4;

/**
 * What the pill says while a dictation or a look at the screen runs (DATA_MODEL.md 13.7): the label and the icon (Lucide `mic` /
 * `scan-eye`); `null` = the ordinary labels of the state.
 */
export function activityOf(data: VoicePillEvent): { key: "voice.dictating" | "voice.looking"; icon: "mic" | "scan-eye" } | null {
  if (data.activity === "dictate" && (data.state === "listening" || data.state === "recognizing" || data.state === "thinking")) return { key: "voice.dictating", icon: "mic" };
  if (data.activity === "screen" && (data.state === "thinking" || data.state === "loading")) return { key: "voice.looking", icon: "scan-eye" };
  return null;
}

/** The two icons, as Lucide draws them (24x24 line icons). */
export function ActivityIcon({ icon }: { icon: "mic" | "scan-eye" }) {
  return (
    <svg className="voice-activity-icon" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
      {icon === "mic" ? (
        <>
          <path d="M12 19v3" />
          <path d="M19 10v2a7 7 0 0 1-14 0v-2" />
          <rect x="9" y="2" width="6" height="13" rx="3" />
        </>
      ) : (
        <>
          <path d="M3 7V5a2 2 0 0 1 2-2h2" />
          <path d="M17 3h2a2 2 0 0 1 2 2v2" />
          <path d="M21 17v2a2 2 0 0 1-2 2h-2" />
          <path d="M7 21H5a2 2 0 0 1-2-2v-2" />
          <circle cx="12" cy="12" r="1" />
          <path d="M18.944 12.33a1 1 0 0 0 0-.66 7.5 7.5 0 0 0-13.888 0 1 1 0 0 0 0 .66 7.5 7.5 0 0 0 13.888 0" />
        </>
      )}
    </svg>
  );
}

export function waveHeights(level: number): number[] {
  const l = Math.max(0, Math.min(1, Number.isFinite(level) ? level : 0));
  return WAVE_COEFFICIENTS.map((c) => Math.max(WAVE_MIN_PX, Math.round(l * WAVE_MAX_PX * c)));
}

/**
 * Floating Voice Pill window component for PC (DESIGN.md §4.12, DATA_MODEL.md §12.6).
 * Receives data via the `voice-pill` Tauri/window event.
 */
export default function VoicePill({ initialData, onConfirm }: VoicePillProps) {
  const [data, setData] = useState<VoicePillEvent>(initialData ?? DEFAULT_EVENT);

  useEffect(() => {
    let active = true;
    let off: (() => void) | null = null;

    onVoicePill((ev) => {
      if (active) setData(ev);
    }).then(
      (unlisten) => {
        if (active) off = unlisten;
        else unlisten();
      },
      () => {},
    );

    return () => {
      active = false;
      off?.();
    };
  }, []);

  async function handleConfirm(yes: boolean) {
    if (onConfirm) {
      onConfirm(yes);
    } else if (data.id) {
      try {
        await call("voice_confirm", { id: data.id, yes });
      } catch {
        // ignore confirmation dispatch error
      }
    }
  }

  // Keyboard navigation for confirm state: Enter = Yes, Esc = No (DESIGN.md §4.12)
  useEffect(() => {
    if (data.state !== "confirm") return;
    const onKeyDown = (e: KeyboardEvent) => {
      if (e.key === "Enter") {
        e.preventDefault();
        void handleConfirm(true);
      } else if (e.key === "Escape") {
        e.preventDefault();
        void handleConfirm(false);
      }
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [data.state, data.id]);

  const reply = tVoice(data.replyKey, data.replyArgs, data.reply);
  const confirmText = tVoice(data.confirm?.key, data.confirm?.args, data.confirm?.text);
  const barHeights = waveHeights(data.level ?? 0);
  const listening = data.state === "listening";
  // Words already heard win over the label of a dictation.
  const activity = data.state === "listening" && data.heard ? null : activityOf(data);

  // The click on the stop button, or anywhere on the pill while it listens, ends the recording (DATA_MODEL.md 12.6a).
  async function handleStop() {
    if (!listening) return;
    try {
      await call("voice_stop", {});
    } catch {
      // the pill is closing or the agent is gone
    }
  }

  return (
    <div className="voice-pill-container">
      <div
        className={`voice-pill${listening ? " voice-pill-listening" : ""}`}
        role="status"
        aria-live="polite"
        onClick={listening ? () => void handleStop() : undefined}
      >
        <img src={logoUrl} width={20} height={20} alt="Decka" className="voice-pill-logo" />

        {/* 5 live waveform bars */}
        <div className="voice-wave" aria-hidden="true">
          {barHeights.map((h, i) => (
            <span key={i} className="voice-wave-bar" style={{ height: `${h}px` }} />
          ))}
        </div>

        {/* Main status text */}
        <div className="voice-pill-text">
          {activity && (
            <span className="voice-status-text voice-activity">
              <ActivityIcon icon={activity.icon} /> {t(activity.key)}
            </span>
          )}

          {data.state === "listening" && !activity && (
            <span>
              {data.heard ? (
                <span className="voice-heard">{data.heard}</span>
              ) : (
                <span className="voice-status-text">{t("voice.listening")}</span>
              )}
            </span>
          )}

          {data.state === "recognizing" && !activity && (
            <span className="voice-thinking">{t("voice.recognizing")}</span>
          )}

          {data.state === "thinking" && !activity && (
            <span className="voice-thinking">{t("voice.thinking")}</span>
          )}

          {data.state === "done" && (
            <span className="voice-done">
              <span className="voice-check" aria-hidden="true">✓ </span>
              {reply || data.heard || t("voice.done")}
            </span>
          )}

          {data.state === "error" && (
            <span className="voice-error">
              <span className="voice-error-dot" aria-hidden="true">● </span>
              {reply || t("voice.error")}
            </span>
          )}

          {data.state === "confirm" && (
            <span className="voice-confirm-prompt">
              {confirmText || reply || t("voice.confirm_title")}
            </span>
          )}

          {data.state === "idle" && (
            <span className="voice-status-text">{t("voice.ready", { hotkey: data.hotkey ?? "" })}</span>
          )}

          {data.state === "loading" && (
            <span className="voice-status-text">{t("voice.loading")}</span>
          )}
        </div>

        {listening && (
          <button
            type="button"
            className="voice-stop"
            title={t("voice.stop_hint")}
            aria-label={t("voice.stop_hint")}
            onClick={(e) => {
              e.stopPropagation();
              void handleStop();
            }}
          >
            {/* Phosphor "stop" (fill) */}
            <svg width="16" height="16" viewBox="0 0 256 256" fill="currentColor" aria-hidden="true">
              <path d="M208,32H48A16,16,0,0,0,32,48V208a16,16,0,0,0,16,16H208a16,16,0,0,0,16-16V48A16,16,0,0,0,208,32Z" />
            </svg>
          </button>
        )}
      </div>

      {/* Confirmation sheet expansion (DESIGN.md §4.12) */}
      {data.state === "confirm" && (
        <div className="voice-pill-confirm">
          <div className="voice-confirm-text">
            {confirmText || reply || t("voice.confirm_title")}
          </div>
          <div className="voice-confirm-actions">
            <button
              type="button"
              className="voice-btn voice-btn-no"
              onClick={() => void handleConfirm(false)}
            >
              {t("voice.no")} <span className="voice-btn-key">(Esc)</span>
            </button>
            <button
              type="button"
              className="voice-btn voice-btn-yes"
              onClick={() => void handleConfirm(true)}
            >
              {t("voice.yes")} <span className="voice-btn-key">(Enter)</span>
            </button>
          </div>
        </div>
      )}
    </div>
  );
}
