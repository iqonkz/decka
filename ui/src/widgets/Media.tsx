import { useEffect, useId, useRef, useState } from "react";
import { t, type MediaInfo, type MediaItem } from "@decka/shared";
import { useDeckEnv } from "../deckEnv";
import Icon from "../Icon";
import Key from "../Key";
import { useDeckMetrics } from "../metricsContext";
import { coverGradient, sampleImageColor } from "./coverColor";
import { effectiveMediaPositionMs, formatOffsetSeconds, selectActiveLyricIndex } from "./lyricsLogic";
import type { LyricsInfo } from "@decka/shared";
import { mediaKeySet } from "./mediaKeys";
import { currentPosition, formatTime } from "./mediaPos";
import WidgetEmpty from "./WidgetEmpty";

const CONTROL_SIDE = 40;
const LYRICS_HOLD_MS = 400;
const LYRICS_BAR_MS = 4000;
const MOVE_CANCEL_PX = 8;

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

/** Right-side lyrics panel for `media` widget L (DESIGN.md 4.17): synced / plain view + long-press offset pills. */
function LyricsPanel({
  lyrics,
  media,
  now,
  onOffset,
}: {
  lyrics: LyricsInfo;
  media: MediaInfo;
  now: number;
  onOffset?: (trackKey: string, deltaMs: -500 | 500) => void;
}) {
  const [optimistic, setOptimistic] = useState<{ trackKey: string; baseOffset: number; delta: number }>({
    trackKey: lyrics.trackKey,
    baseOffset: lyrics.offsetMs,
    delta: 0,
  });
  const localDelta =
    optimistic.trackKey === lyrics.trackKey && optimistic.baseOffset === lyrics.offsetMs
      ? optimistic.delta
      : 0;
  const effectiveOffset = lyrics.offsetMs + localDelta;

  const [showOffset, setShowOffset] = useState(false);
  const holdTimer = useRef<number | null>(null);
  const hideTimer = useRef<number | null>(null);
  const pressOrigin = useRef<{ x: number; y: number } | null>(null);

  const viewportRef = useRef<HTMLDivElement>(null);
  const lineRefs = useRef<Array<HTMLDivElement | null>>([]);
  const [shiftY, setShiftY] = useState(0);

  const effectiveMs = effectiveMediaPositionMs(media, now, effectiveOffset);
  const activeIdx = lyrics.synced ? selectActiveLyricIndex(lyrics.lines, effectiveMs) : -1;

  // Center the active synced line around 40% of viewport height (DESIGN.md 4.17).
  useEffect(() => {
    if (!lyrics.synced) return;
    const vp = viewportRef.current;
    if (!vp) return;
    const anchor = vp.clientHeight * 0.4;
    if (activeIdx < 0) {
      setShiftY(Math.max(0, Math.round(anchor - 14)));
      return;
    }
    const lineEl = lineRefs.current[activeIdx];
    if (!lineEl) return;
    setShiftY(Math.round(anchor - (lineEl.offsetTop + lineEl.offsetHeight / 2)));
  }, [lyrics.synced, lyrics.trackKey, lyrics.lines.length, activeIdx]);

  useEffect(() => {
    return () => {
      if (holdTimer.current !== null) window.clearTimeout(holdTimer.current);
      if (hideTimer.current !== null) window.clearTimeout(hideTimer.current);
    };
  }, []);

  const scheduleHideBar = () => {
    if (hideTimer.current !== null) window.clearTimeout(hideTimer.current);
    hideTimer.current = window.setTimeout(() => setShowOffset(false), LYRICS_BAR_MS);
  };

  const clearHold = () => {
    if (holdTimer.current !== null) {
      window.clearTimeout(holdTimer.current);
      holdTimer.current = null;
    }
    pressOrigin.current = null;
  };

  const onPointerDown = (e: React.PointerEvent<HTMLDivElement>) => {
    if ((e.target as HTMLElement).closest(".media-lyrics-offset")) return;
    pressOrigin.current = { x: e.clientX, y: e.clientY };
    clearHold();
    pressOrigin.current = { x: e.clientX, y: e.clientY };
    holdTimer.current = window.setTimeout(() => {
      holdTimer.current = null;
      setShowOffset(true);
      scheduleHideBar();
    }, LYRICS_HOLD_MS);
  };

  const onPointerMove = (e: React.PointerEvent<HTMLDivElement>) => {
    const start = pressOrigin.current;
    if (!start || holdTimer.current === null) return;
    if (Math.hypot(e.clientX - start.x, e.clientY - start.y) > MOVE_CANCEL_PX) {
      clearHold();
    }
  };

  const applyDelta = (deltaMs: -500 | 500) => {
    setOptimistic({
      trackKey: lyrics.trackKey,
      baseOffset: lyrics.offsetMs,
      delta: localDelta + deltaMs,
    });
    onOffset?.(lyrics.trackKey, deltaMs);
    scheduleHideBar();
  };

  return (
    <div
      className="media-lyrics w-l"
      aria-label={t("lyrics.region")}
      onPointerDown={onPointerDown}
      onPointerMove={onPointerMove}
      onPointerUp={clearHold}
      onPointerCancel={clearHold}
      onPointerLeave={clearHold}
    >
      {lyrics.synced ? (
        <div ref={viewportRef} className="media-lyrics-viewport media-lyrics-masked">
          <div className="media-lyrics-track" style={{ transform: `translateY(${shiftY}px)` }}>
            {lyrics.lines.map((line, i) => (
              <div
                key={`${i}-${line.t ?? "x"}`}
                ref={(el) => {
                  lineRefs.current[i] = el;
                }}
                className={i === activeIdx ? "media-lyric-line active" : "media-lyric-line"}
              >
                {line.text || "\u00A0"}
              </div>
            ))}
          </div>
        </div>
      ) : (
        <div className="media-lyrics-plain media-lyrics-masked">
          {lyrics.lines.map((line, i) => (
            <div key={i} className="media-lyric-plain-line">
              {line.text || "\u00A0"}
            </div>
          ))}
        </div>
      )}

      {showOffset && (
        <div className="media-lyrics-offset" onPointerDown={(e) => e.stopPropagation()}>
          <button
            type="button"
            className="media-offset-pill"
            aria-label={t("lyrics.earlier")}
            onClick={() => applyDelta(-500)}
          >
            {t("lyrics.step_minus")}
          </button>
          <span className="media-offset-value">{t("lyrics.offset_sec", { val: formatOffsetSeconds(effectiveOffset) })}</span>
          <button
            type="button"
            className="media-offset-pill"
            aria-label={t("lyrics.later")}
            onClick={() => applyDelta(500)}
          >
            {t("lyrics.step_plus")}
          </button>
        </div>
      )}
    </div>
  );
}

/** "Now playing" (DATA_MODEL.md 6.2, DESIGN.md 4.16, 4.17): cover gradient, wavy progress, 40 px controls, L lyrics. */
export default function Media({ item }: { item: MediaItem }) {
  const { media, lyrics, onLyricsOffset } = useDeckEnv();
  const { side, gap } = useDeckMetrics();
  const [ref, size] = useSize();
  const stacked = isStacked(item);
  const playing = media?.playing === true;
  const hasLyrics = Boolean(lyrics && lyrics.lines.length > 0 && item.w >= 4 && item.h >= 2);
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

  // Fast 250 ms tick when synced lyrics play; 1000 ms otherwise; stopped when paused.
  const [now, setNow] = useState(() => Date.now());
  const syncedPlaying = playing && hasLyrics && lyrics?.synced === true;
  useEffect(() => {
    setNow(Date.now());
    if (!playing) return;
    const timer = window.setInterval(() => setNow(Date.now()), syncedPlaying ? 250 : 1000);
    return () => window.clearInterval(timer);
  }, [playing, syncedPlaying, media]);

  if (!media) {
    return <WidgetEmpty icon="lucide:music" text={t("widget.nothing_playing")} />;
  }

  const cover = hasLyrics ? Math.min(48, Math.max(36, Math.round(size.height * 0.34))) : coverSide(stacked, size);
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
      <div
        ref={ref}
        className={[
          stacked && !hasLyrics ? "media stacked" : "media",
          hasLyrics ? "has-lyrics" : "",
          coverBg ? "has-cover-bg" : "",
        ].filter(Boolean).join(" ")}
      >
        {hasLyrics && lyrics ? (
          <>
            <div className="media-player">
              <div className="media-player-top">
                {cover > 0 &&
                  (media.art ? (
                    <img className="media-art" src={media.art} alt="" style={{ width: cover, height: cover }} draggable={false} />
                  ) : (
                    <div className="media-art media-art-empty" style={{ width: cover, height: cover }}>
                      <Icon icon="lucide:music" size={Math.round(0.45 * cover)} />
                    </div>
                  ))}
                <div className="media-text">
                  <div className="media-title two-lines">{media.title}</div>
                  {media.artist && <div className="media-artist">{media.artist}</div>}
                </div>
              </div>
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
              {controls}
            </div>
            <LyricsPanel lyrics={lyrics} media={media} now={now} onOffset={onLyricsOffset} />
          </>
        ) : (
          <>
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
          </>
        )}
      </div>
    </>
  );
}
