import { useEffect, useState } from "react";
import { t, type TwitchItem } from "@decka/shared";
import { useDeckEnv } from "../deckEnv";
import { obsElapsed } from "./obsTime";

/**
 * The Twitch widget: the dot and `LIVE` with the viewers, the followers, and (from 3 wide) the title, the game and the time on
 * air. A look at the channel that failed leaves the last numbers with the word `No connection`.
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
    return (
      <div className="twitch twitch-empty">
        <span className="twitch-dot off" />
        <span className="twitch-text">{t("twitch.off")}</span>
      </div>
    );
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
      {big && twitch.live && twitch.title !== undefined && <div className="twitch-title">{twitch.title}</div>}
      {big && twitch.live && twitch.game !== undefined && <div className="twitch-game">{twitch.game}</div>}
    </div>
  );
}
