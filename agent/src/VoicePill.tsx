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
          {data.state === "listening" && (
            <span>
              {data.heard ? (
                <span className="voice-heard">{data.heard}</span>
              ) : (
                <span className="voice-status-text">{t("voice.listening")}</span>
              )}
            </span>
          )}

          {data.state === "thinking" && (
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
