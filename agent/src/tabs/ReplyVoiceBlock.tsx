import { useEffect, useState } from "react";
import { t } from "@decka/shared";
import { Group, Row, SectionHeader } from "../ui/SettingsGroup";

/** The speed of speech in percent of the normal one, as the PC keeps it (`voice_set`). */
export const MIN_SPEECH_PERCENT = 80;
export const MAX_SPEECH_PERCENT = 130;
export const DEFAULT_SPEECH_PERCENT = 105;
const STEP = 5;

export interface ReplyVoiceBlockProps {
  percent: number;
  onChange: (percent: number) => void;
}

/** T33.1b, T33.2: the voice that reads the answers (the one of Windows) and how fast it speaks. */
export default function ReplyVoiceBlock({ percent, onChange }: ReplyVoiceBlockProps) {
  // The slider moves at once; the PC is told when it is let go.
  const [shown, setShown] = useState(percent);
  useEffect(() => setShown(percent), [percent]);
  const save = () => {
    if (shown !== percent) onChange(shown);
  };

  return (
    <div id="voice-reply-voice" style={{ marginTop: 16 }}>
      <SectionHeader title={t("m33.reply_voice_title")} />
      <Group>
        <Row
          id="section-reply-speed"
          title={t("m33.speed_label")}
          description={
            <span style={{ display: "inline-flex", alignItems: "center", gap: 8, flexWrap: "wrap" }}>
              <span>{t("m33.engine_windows_hint")}</span>
              {percent !== DEFAULT_SPEECH_PERCENT && (
                <button
                  type="button"
                  onClick={() => onChange(DEFAULT_SPEECH_PERCENT)}
                  style={{ background: "none", border: 0, padding: 0, color: "var(--accent)", cursor: "pointer", font: "inherit", fontSize: "var(--font-xs)" }}
                >
                  {t("m33.speed_reset")}
                </button>
              )}
            </span>
          }
          icon="gauge"
        >
          <div style={{ display: "flex", alignItems: "center", gap: 10 }}>
            <input
              type="range"
              aria-label={t("m33.speed_label")}
              min={MIN_SPEECH_PERCENT}
              max={MAX_SPEECH_PERCENT}
              step={STEP}
              value={shown}
              onChange={(e) => setShown(Number(e.target.value))}
              onPointerUp={save}
              onKeyUp={save}
              onBlur={save}
              style={{ accentColor: "var(--accent)" }}
            />
            <span style={{ fontSize: "var(--font-sm)", fontVariantNumeric: "tabular-nums", minWidth: 44, textAlign: "right", color: "var(--text-muted)" }}>
              {(shown / 100).toFixed(2)}×
            </span>
          </div>
        </Row>
      </Group>
    </div>
  );
}
