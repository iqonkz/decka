import { useEffect, useState } from "react";
import { t, type YoutubeItem } from "@decka/shared";
import { useDeckEnv } from "../deckEnv";
import { obsElapsed } from "./obsTime";
import WidgetEmpty from "./WidgetEmpty";

/**
 * The YouTube widget (DESIGN.md 4.16).
 */
export default function Youtube({ item }: { item: YoutubeItem }) {
  const { youtube } = useDeckEnv();
  const timed = youtube?.live === true && youtube.startedAt !== undefined && item.w >= 3;

  const [now, setNow] = useState(() => Date.now());
  useEffect(() => {
    setNow(Date.now());
    if (!timed) return;
    const id = window.setInterval(() => setNow(Date.now()), 1000);
    return () => window.clearInterval(id);
  }, [timed]);

  if (!youtube) {
    return <WidgetEmpty icon="lucide:play" text={t("youtube.off")} />;
  }
  const big = item.h >= 2;
  return (
    <div className={big ? "twitch big" : "twitch"}>
      <div className="twitch-head">
        <span className={youtube.live ? "twitch-dot live" : "twitch-dot off"} />
        <strong className="twitch-state">{youtube.live ? t("twitch.live") : t("twitch.offline")}</strong>
        {youtube.live && <span className="twitch-viewers">{t("twitch.viewers", { n: youtube.viewers })}</span>}
        {timed && youtube.startedAt !== undefined && <span className="twitch-time">{obsElapsed(youtube.startedAt, now)}</span>}
      </div>
      <div className="twitch-line">
        {youtube.subscribers !== undefined ? t("youtube.subscribers", { n: youtube.subscribers }) : youtube.channel}
        {!youtube.connected && <span className="twitch-stale"> · {t("twitch.no_connection")}</span>}
      </div>
      {youtube.live && youtube.title !== undefined && <div className="twitch-title w-m">{youtube.title}</div>}
    </div>
  );
}
