import { useEffect, useRef, useState } from "react";
import type { MediaItem } from "@decka/shared";
import { useAppState } from "../../store/store";
import Icon from "../Icon";
import Key from "../Key";
import { useDeckMetrics } from "../metricsContext";
import { currentPosition, formatTime } from "./mediaPos";

const KEY_RATIO = 0.55;

/** Height in px of the element behind the returned ref; follows resizes. */
function useHeight(): [React.RefObject<HTMLDivElement | null>, number] {
  const ref = useRef<HTMLDivElement>(null);
  const [height, setHeight] = useState(0);
  useEffect(() => {
    const el = ref.current;
    if (!el) return;
    const measure = () => setHeight(el.clientHeight);
    measure();
    const observer = new ResizeObserver(measure);
    observer.observe(el);
    return () => observer.disconnect();
  }, []);
  return [ref, height];
}

/** "Now playing" (DATA_MODEL.md 6.2): cover, title, artist, progress and the three transport keys. */
export default function Media({ item }: { item: MediaItem }) {
  const media = useAppState((s) => s.state?.media ?? null);
  const { side } = useDeckMetrics();
  const [ref, height] = useHeight();
  const playing = media?.playing === true;

  // The position is calculated here; once a second, and only while playing.
  const [now, setNow] = useState(() => Date.now());
  useEffect(() => {
    setNow(Date.now());
    if (!playing) return;
    const timer = window.setInterval(() => setNow(Date.now()), 1000);
    return () => window.clearInterval(timer);
  }, [playing, media]);

  const cover = Math.max(0, height);
  const keySide = Math.round(KEY_RATIO * side);
  const position = media ? currentPosition(media, now) : null;
  const progress = media?.durationSec && position !== null ? Math.min(1, position / media.durationSec) : null;

  const keys: { action: string | undefined; icon: string; label: string }[] = [];
  if (item.w >= 3) keys.push({ action: item.prevAction, icon: "lucide:skip-back", label: "Previous" });
  keys.push({ action: item.playAction, icon: playing ? "lucide:pause" : "lucide:play", label: playing ? "Pause" : "Play" });
  if (item.w >= 3) keys.push({ action: item.nextAction, icon: "lucide:skip-forward", label: "Next" });
  const controls = (
    <div className="media-keys">
      {keys.map((k) => k.action !== undefined && <Key key={k.label} actionId={k.action} icon={k.icon} side={keySide} />)}
    </div>
  );
  const below = item.h >= 2;

  return (
    <div ref={ref} className="media">
      {cover > 0 &&
        (media?.art ? (
          <img className="media-art" src={media.art} alt="" style={{ width: cover, height: cover }} draggable={false} />
        ) : (
          <div className="media-art media-art-empty" style={{ width: cover, height: cover }}>
            <Icon icon="lucide:music" size={Math.round(0.45 * cover)} />
          </div>
        ))}
      <div className="media-text">
        {media ? (
          <>
            <div className={below ? "media-title two-lines" : "media-title"}>{media.title}</div>
            {media.artist && <div className="media-artist">{media.artist}</div>}
          </>
        ) : (
          <div className="media-nothing">Nothing playing</div>
        )}
        {progress !== null && (
          <div
            className="media-progress"
            role="progressbar"
            aria-valuemin={0}
            aria-valuemax={media?.durationSec}
            aria-valuenow={position ?? 0}
            aria-valuetext={`${formatTime(position ?? 0)} of ${formatTime(media?.durationSec ?? 0)}`}
          >
            <div className="media-progress-fill" style={{ width: `${progress * 100}%` }} />
          </div>
        )}
        {below && controls}
      </div>
      {!below && controls}
    </div>
  );
}
