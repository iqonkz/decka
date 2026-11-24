import { t, tVoice } from "@decka/shared";
import { confirmVoice, useVoiceState } from "./api";

export default function VoiceConfirmSheet() {
  const voice = useVoiceState();
  const confirm = voice?.confirm;
  const isConfirm = voice?.state === "confirm" || confirm !== undefined;

  if (!isConfirm || !confirm) {
    return null;
  }

  return (
    <div className="menu-backdrop" onPointerDown={() => confirmVoice(false)}>
      <div
        className="menu-sheet voice-confirm-sheet"
        role="dialog"
        aria-label={t("voice.confirm_title")}
        onPointerDown={(e) => e.stopPropagation()}
      >
        <div className="sheet-handle" aria-hidden="true" />
        <div className="phone-group-title">{t("voice.confirm_title")}</div>
        <h2 className="sheet-title">{tVoice(confirm.key, confirm.args, confirm.text)}</h2>
        <div className="service-actions" style={{ marginTop: 12 }}>
          <button
            type="button"
            className="pill-btn primary"
            onClick={() => confirmVoice(true)}
          >
            {t("voice.yes")}
          </button>
          <button
            type="button"
            className="pill-btn secondary"
            onClick={() => confirmVoice(false)}
          >
            {t("voice.no")}
          </button>
        </div>
      </div>
    </div>
  );
}
