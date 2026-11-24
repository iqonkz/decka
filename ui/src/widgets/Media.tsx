import { useEffect, useId, useRef, useState } from "react";
import { t, type MediaItem } from "@decka/shared";
import { useDeckEnv } from "../deckEnv";
import Icon from "../Icon";
import Key from "../Key";
import { useDeckMetrics } from "../metricsContext";
import { coverGradient, sampleImageColor } from "./coverColor";
import { mediaKeySet } from "./mediaKeys";
import { currentPosition, formatTime } from "./mediaPos";
import WidgetEmpty from "./WidgetEmpty";

const CONTROL_SIDE = 40;

/** Width and height in px of the element behind the returned ref; follows resizes. */
function useSize(): [React.RefObject<HTMLDivElement | null>, { width: number; height: number }] {
  const ref = useRef<HTMLDivElement>(null);
  const [size, setSize] = useState({ width: 0, height: 0 });
  useEffect(() => {
    const el = ref.current;
    if (!el) return;
    const measure = () => setSize({ width: el.clientWidth, height: el.clientHeight });
    measure();
    const observer = new ResizeObserver(measure);
    observer.observe(el);
    return () => observer.disconnect();
  }, []);
  return [ref, size];
}

/**
 * Narrow and tall (`w <= 2`, `h >= 2`, as in a portrait deck): the cover on top, the text under it (DESIGN.md 2.2).
 * The cover is as wide as the widget allows but never takes more than the upper half.
 */
export function isStacked(item: { w: number; h: number }): boolean {
  return item.w <= 2 && item.h >= 2;
}

/** Side of the cover: the whole height in a row; in a stack the width, at most half the height. */
export function coverSide(stacked: boolean, size: { width: number; height: number }): number {
  return Math.max(0, stacked ? Math.min(size.width, Math.round(size.height * 0.5)) : size.height);
}

/** SVG sine wave progress bar (DESIGN.md 4.16): 2 px amplitude, scrolls only while playing; straight in reduced mode. */
function WavyProgress({ progress, playing, posText, totalText, durationSec, positionSec }: {
  progress: number;
  playing: boolean;
  posText: string;
  totalText: string;
  durationSec?: number;
  positionSec: number;
}) {
  const clipId = useId();
  const pct = Math.min(100, Math.max(0, progress * 100));
  // Repeating period of 16 px; height 8 px with center at y=4 and amplitude +/- 2 px.
  const segments = 18;
  let d = "M 0 4";
  for (let i = 0; i < segments; i++) {
    const x0 = i * 16;
    d += ` Q ${x0 + 4} 2, ${x0 + 8} 4 Q ${x0 + 12} 6, ${x0 + 16} 4`;
  }
  const width = segments * 16;
  return (
    <div
      className={playing ? "media-progress playing" : "media-progress"}
      role="progressbar"
      aria-valuemin={0}
      aria-valuemax={durationSec}
      aria-valuenow={positionSec}
      aria-valuetext={t("media.progress", { pos: posText, total: totalText })}
    >
      <div className="media-progress-straight">
        <div className="media-progress-fill" style={{ width: `${pct}%` }} />
      </div>
      <svg className="media-progress-wave" viewBox={`0 0 ${width - 16} 8`} preserveAspectRatio="none" aria-hidden="true">
        <defs>
          <clipPath id={clipId}>
            <rect x="0" y="0" width={`${pct}%`} height="8" />
          </clipPath>
        </defs>
        <path className="media-wave-track" d={d} fill="none" strokeWidth="2" strokeLinecap="round" />
        <g clipPath={`url(#${clipId})`}>
          <path className="media-wave-fill" d={d} fill="none" strokeWidth="2" strokeLinecap="round" />
        </g>
      </svg>
    </div>
  );
}

/** "Now playing" (DATA_MODEL.md 6.2, DESIGN.md 4.16): cover gradient, wavy progress, 40 px controls. */
export default function Media({ item }: { item: MediaItem }) {
  const { media } = useDeckEnv();
  const { side, gap } = useDeckMetrics();
  const [ref, size] = useSize();
  const stacked = isStacked(item);
  const playing = media?.playing === true;
  const [coverBg, setCoverBg] = useState<string | null>(null);

  useEffect(() => {
    const art = media?.art;
    if (!art) {
      setCoverBg(null);
      return;
    }
    let live = true;
    void sampleImageColor(art).then((rgb) => {
      if (live) setCoverBg(rgb ? coverGradient(rgb) : null);
    });
    return () => { live = false; };
  }, [media?.art]);

  // The position is calculated here; once a second, and only while playing.
  const [now, setNow] = useState(() => Date.now());
  useEffect(() => {
    setNow(Date.now());
    if (!playing) return;
    const timer = window.setInterval(() => setNow(Date.now()), 1000);
    return () => window.clearInterval(timer);
  }, [playing, media]);

  if (!media) {
    return <WidgetEmpty icon="lucide:music" text={t("widget.nothing_playing")} />;
  }

  const cover = coverSide(stacked, size);
  const position = currentPosition(media, now);
  const progress = media.durationSec && position !== null ? Math.min(1, position / media.durationSec) : null;
  const posText = formatTime(position ?? 0);
  const totalText = formatTime(media.durationSec ?? 0);

  const all = {
    prev: { action: item.prevAction, icon: "lucide:skip-back", label: t("media.previous") },
    play: { action: item.playAction, icon: playing ? "lucide:pause" : "lucide:play", label: playing ? t("media.pause") : t("media.play") },
    next: { action: item.nextAction, icon: "lucide:skip-forward", label: t("media.next") },
  };
  const names = mediaKeySet(item.w, item.h, side, gap);
  const controls = (
    <div className="media-keys">
      {names.map((name) => {
        const k = all[name];
        return k.action !== undefined && <Key key={name} actionId={k.action} icon={k.icon} side={CONTROL_SIDE} />;
      })}
    </div>
  );
  const below = item.h >= 2;

  return (
    <>
      {coverBg && <div className="media-bg" style={{ background: coverBg }} aria-hidden="true" />}
      <div ref={ref} className={[stacked ? "media stacked" : "media", coverBg ? "has-cover-bg" : ""].filter(Boolean).join(" ")}>
        {cover > 0 &&
          (media.art ? (
            <img className="media-art" src={media.art} alt="" style={{ width: cover, height: cover }} draggable={false} />
          ) : (
            <div className="media-art media-art-empty" style={{ width: cover, height: cover }}>
              <Icon icon="lucide:music" size={Math.round(0.45 * cover)} />
            </div>
          ))}
        <div className="media-text">
          <div className={below ? "media-title two-lines" : "media-title"}>{media.title}</div>
          {media.artist && <div className="media-artist w-m">{media.artist}</div>}
          {media.album && <div className="media-album w-l">{media.album}</div>}
          {progress !== null && (
            <>
              <WavyProgress
                progress={progress}
                playing={playing}
                posText={posText}
                totalText={totalText}
                durationSec={media.durationSec}
                positionSec={position ?? 0}
              />
              <div className="media-times w-l">
                <span>{posText}</span>
                <span>{totalText}</span>
              </div>
            </>
          )}
          {below && controls}
        </div>
        {!below && controls}
      </div>
    </>
  );
}
