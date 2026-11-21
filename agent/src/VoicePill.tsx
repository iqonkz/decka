import { useEffect, useState } from "react";
import logoUrl from "../../docs/assets/decka-logo.svg";
import { call, onVoicePill, type VoicePillEvent } from "./ipc";

export interface VoicePillProps {
  initialData?: VoicePillEvent;
  onConfirm?: (yes: boolean) => void;
}

const DEFAULT_EVENT: VoicePillEvent = {
  state: "listening",
  level: 0.2,
};

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

  const level = Math.max(0, Math.min(1, data.level ?? (data.state === "listening" ? 0.3 : 0.05)));

  // Waveform 5 bars scaled by level
  const barHeights = [
    Math.max(4, Math.round(level * 16 * 0.5)),
    Math.max(4, Math.round(level * 20 * 0.8)),
    Math.max(4, Math.round(level * 22 * 1.0)),
    Math.max(4, Math.round(level * 20 * 0.75)),
    Math.max(4, Math.round(level * 16 * 0.45)),
  ];

  return (
    <div className="voice-pill-container">
      <div className="voice-pill" role="status" aria-live="polite">
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
                <span className="voice-status-text">Listening…</span>
              )}
            </span>
          )}

          {data.state === "thinking" && (
            <span className="voice-thinking">
              Thinking<span className="voice-dots">…</span>
            </span>
          )}

          {data.state === "done" && (
            <span className="voice-done">
              <span className="voice-check" aria-hidden="true">✓ </span>
              {data.reply || data.heard || "Done"}
            </span>
          )}

          {data.state === "error" && (
            <span className="voice-error">
              <span className="voice-error-dot" aria-hidden="true">● </span>
              {data.reply || "Didn't catch that"}
            </span>
          )}

          {data.state === "confirm" && (
            <span className="voice-confirm-prompt">
              {data.confirm?.text || data.reply || "Confirm action"}
            </span>
          )}

          {data.state === "idle" && (
            <span className="voice-status-text">Ready</span>
          )}

          {data.state === "loading" && (
            <span className="voice-status-text">Loading…</span>
          )}
        </div>
      </div>

      {/* Confirmation sheet expansion (DESIGN.md §4.12) */}
      {data.state === "confirm" && (
        <div className="voice-pill-confirm">
          <div className="voice-confirm-text">
            {data.confirm?.text || data.reply || "Confirm this action?"}
          </div>
          <div className="voice-confirm-actions">
            <button
              type="button"
              className="voice-btn voice-btn-no"
              onClick={() => void handleConfirm(false)}
            >
              No <span className="voice-btn-key">(Esc)</span>
            </button>
            <button
              type="button"
              className="voice-btn voice-btn-yes"
              onClick={() => void handleConfirm(true)}
            >
              Yes <span className="voice-btn-key">(Enter)</span>
            </button>
          </div>
        </div>
      )}
    </div>
  );
}
