import { useEffect, useState } from "react";
import { t, type TwitchItem } from "@decka/shared";
import { useDeckEnv } from "../deckEnv";
import { obsElapsed } from "./obsTime";
import WidgetEmpty from "./WidgetEmpty";

/**
 * The Twitch widget (DESIGN.md 4.16).
 */
export default function Twitch({ item }: { item: TwitchItem }) {
  const { twitch } = useDeckEnv();
  const timed = twitch?.live === true && twitch.startedAt !== undefined && item.w >= 3;

  // One tick a second, only while the time on air is shown.
  const [now, setNow] = useState(() => Date.now());
  useEffect(() => {
    setNow(Date.now());
    if (!timed) return;
    const id = window.setInterval(() => setNow(Date.now()), 1000);
    return () => window.clearInterval(id);
  }, [timed]);

  if (!twitch) {
    return <WidgetEmpty icon="lucide:video" text={t("twitch.off")} />;
  }
  const big = item.h >= 2;
  return (
    <div className={big ? "twitch big" : "twitch"}>
      <div className="twitch-head">
        <span className={twitch.live ? "twitch-dot live" : "twitch-dot off"} />
        <strong className="twitch-state">{twitch.live ? t("twitch.live") : t("twitch.offline")}</strong>
        {twitch.live && <span className="twitch-viewers">{t("twitch.viewers", { n: twitch.viewers })}</span>}
        {timed && twitch.startedAt !== undefined && <span className="twitch-time">{obsElapsed(twitch.startedAt, now)}</span>}
      </div>
      <div className="twitch-line muted-line">
        {t("twitch.followers", { n: twitch.followers })}
        {!twitch.connected && <span className="twitch-stale"> · {t("twitch.no_connection")}</span>}
      </div>
      {twitch.live && twitch.title !== undefined && <div className="twitch-title w-m">{twitch.title}</div>}
      {twitch.live && twitch.game !== undefined && <div className="twitch-game w-l">{twitch.game}</div>}
    </div>
  );
}
