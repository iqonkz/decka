import { t } from "@decka/shared";
import { confirmVoice, useVoiceState } from "./api";

export default function VoiceConfirmSheet() {
  const voice = useVoiceState();
  const confirm = voice?.confirm;
  const isConfirm = voice?.state === "confirm" || confirm !== undefined;

  if (!isConfirm || !confirm) {
    return null;
  }

  return (
    <div className="perm-backdrop" onPointerDown={() => confirmVoice(false)}>
      <div
        className="perm-sheet voice-confirm-sheet"
        role="dialog"
        aria-label={t("voice.confirm_title")}
        onPointerDown={(e) => e.stopPropagation()}
      >
        <div className="perm-count">{t("voice.confirm_title")}</div>
        <h2 className="perm-title">{confirm.text}</h2>
        <div className="perm-actions" style={{ marginTop: "1.6vmin" }}>
          <button
            type="button"
            className="secondary"
            style={{ flex: 1, height: "56px" }}
            onClick={() => confirmVoice(false)}
          >
            {t("voice.no")}
          </button>
          <button
            type="button"
            className="perm-allow"
            style={{ flex: 1, height: "56px" }}
            onClick={() => confirmVoice(true)}
          >
            {t("voice.yes")}
          </button>
        </div>
      </div>
    </div>
  );
}
