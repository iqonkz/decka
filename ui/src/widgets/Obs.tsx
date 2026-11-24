import { useEffect, useState } from "react";
import { t, type ObsItem } from "@decka/shared";
import { useDeckEnv } from "../deckEnv";
import { obsElapsed } from "./obsTime";
import WidgetEmpty from "./WidgetEmpty";

/** The OBS widget (DATA_MODEL.md 6.4, DESIGN.md 4.16). */
export default function Obs({ item }: { item: ObsItem }) {
  const { obs } = useDeckEnv();
  const running = obs?.connected === true && (obs.recording || obs.streaming);

  // One tick a second, only while something is on air.
  const [now, setNow] = useState(() => Date.now());
  useEffect(() => {
    setNow(Date.now());
    if (!running) return;
    const id = window.setInterval(() => setNow(Date.now()), 1000);
    return () => window.clearInterval(id);
  }, [running]);

  if (!obs || !obs.connected) {
    return <WidgetEmpty icon="lucide:video-off" text={obs ? t("obs.not_connected") : t("obs.off")} />;
  }

  const rec = obs.recording ? (
    <span className="obs-line on">
      <span className="rec-dot" />
      {t("obs.rec")} {obs.recordStartedAt !== undefined ? obsElapsed(obs.recordStartedAt, now) : ""}
    </span>
  ) : (
    <span className="obs-line">{t("obs.not_recording")}</span>
  );
  const live = obs.streaming ? (
    <span className="obs-line on">
      <span className="rec-dot" />
      {t("obs.live")} {obs.streamStartedAt !== undefined ? obsElapsed(obs.streamStartedAt, now) : ""}
    </span>
  ) : (
    <span className="obs-line">{t("obs.offline")}</span>
  );
  const scene = obs.scene && <span className="obs-scene w-m">{obs.scene}</span>;
  return (
    <div className={item.h >= 2 ? "obs big" : "obs"}>
      <span className="obs-dot on" />
      <div className={item.h >= 2 ? "obs-lines stacked" : "obs-lines"}>
        {rec}
        {live}
        {scene}
      </div>
    </div>
  );
}
