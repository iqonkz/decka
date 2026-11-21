import { t } from "@decka/shared";
import VoiceHelp from "./VoiceHelp";

export default function VoiceSection() {
  return (
    <div className="stack">
      <h2>{t("more.voice")}</h2>
      <section className="card">
        <h3>{t("more.voice")}</h3>
        <p className="muted">{t("voice.help_limit")}</p>
      </section>
      <VoiceHelp />
    </div>
  );
}
