import { useEffect, useRef, useState } from "react";
import { t, type VolumeItem } from "@decka/shared";
import { useDeckEnv } from "../deckEnv";
import Icon from "../Icon";
import { useAnimatedNumber } from "./useAnimatedNumber";
import { VOLUME_PRESSES_PER_SECOND, VOLUME_SETTLE_MS, VOLUME_STEP, volumeAt, volumeSteps } from "./volumeSteps";

const clamp = (n: number): number => Math.min(100, Math.max(0, n));

/**
 * The volume slider (DATA_MODEL.md 6.6, DESIGN.md 4.16). Full-width 28 px pill slider with accent fill,
 * value inside the track on the right, and dynamic level icon (`volume-x` / `volume-1` / `volume-2`).
 */
export default function Volume({ item }: { item: VolumeItem }) {
  const { vars, online, onPress, onPressError } = useDeckEnv();
  const raw = vars["audio.volume"];
  const actual = typeof raw === "number" ? clamp(raw) : null;
  const muted = vars["audio.muted"] === true;
  const vertical = item.h > item.w;
  const up = item.upAction ?? "vol-up";
  const down = item.downAction ?? "vol-down";
  const mute = item.muteAction ?? "vol-mute";

  // `expected` is what the slider shows between the first touch and 600 ms after the last press.
  const [expected, setExpected] = useState<number | null>(null);
  const shown = useRef(0);
  const target = useRef(0);
  const loop = useRef<number | null>(null);
  const settle = useRef<number | null>(null);
  const dragging = useRef(false);
  const track = useRef<HTMLDivElement>(null);
  const live = useRef({ onPress, onPressError, up, down });
  live.current = { onPress, onPressError, up, down };

  const stopLoop = () => {
    if (loop.current !== null) window.clearInterval(loop.current);
    loop.current = null;
  };
  const clearSettle = () => {
    if (settle.current !== null) window.clearTimeout(settle.current);
    settle.current = null;
  };
  useEffect(() => () => { stopLoop(); clearSettle(); }, []);

  /** After the last press: show the real value again once it had time to arrive. */
  const settleSoon = () => {
    clearSettle();
    settle.current = window.setTimeout(() => {
      settle.current = null;
      setExpected(null);
    }, VOLUME_SETTLE_MS);
  };

  const step = () => {
    const { action, count } = volumeSteps(shown.current, target.current);
    if (count === 0) {
      stopLoop();
      if (!dragging.current) settleSoon();
      return;
    }
    shown.current = clamp(shown.current + (action === "up" ? VOLUME_STEP : -VOLUME_STEP));
    setExpected(shown.current);
    const send = live.current;
    send.onPress(action === "up" ? send.up : send.down).catch((e: { code: string; message?: string }) => {
      send.onPressError(e);
      stopLoop();
      dragging.current = false;
      settleSoon();
    });
  };

  const move = (e: React.PointerEvent) => {
    const box = track.current?.getBoundingClientRect();
    if (!box) return;
    target.current = volumeAt(vertical ? e.clientY : e.clientX, vertical ? box.top : box.left, vertical ? box.height : box.width, vertical);
    if (loop.current === null) loop.current = window.setInterval(step, Math.round(1000 / VOLUME_PRESSES_PER_SECOND));
  };

  const onDown = (e: React.PointerEvent) => {
    e.stopPropagation(); // a drag on the slider is not a page swipe
    if (actual === null || !online) return;
    (e.currentTarget as HTMLElement).setPointerCapture?.(e.pointerId);
    clearSettle();
    dragging.current = true;
    shown.current = expected ?? actual;
    setExpected(shown.current);
    move(e);
  };
  const onMove = (e: React.PointerEvent) => {
    e.stopPropagation();
    if (dragging.current) move(e);
  };
  const onUp = (e: React.PointerEvent) => {
    e.stopPropagation();
    if (!dragging.current) return;
    dragging.current = false;
    if (loop.current === null) settleSoon(); // it was on target already
  };

  /** A single press for the `-` / `+` at the ends (the slider has no value to drag). */
  const nudge = (actionId: string) => (e: React.PointerEvent) => {
    e.stopPropagation();
    if (online) onPress(actionId).catch(onPressError);
  };

  const value = actual === null ? null : (expected ?? actual);
  const animatedValue = useAnimatedNumber(value ?? 0, 300);
  const fill = value ?? 0;
  const icon = muted || fill === 0 ? "lucide:volume-x" : fill < 50 ? "lucide:volume-1" : "lucide:volume-2";
  const speaker = (
    <button type="button" className="volume-icon" aria-label={t("volume.mute")} onPointerDown={(e) => e.stopPropagation()} onClick={() => online && onPress(mute).catch(onPressError)}>
      <Icon icon={icon} size={20} />
    </button>
  );

  return (
    <div className={["volume", vertical ? "vertical" : "", actual === null ? "no-data" : "", muted ? "muted" : ""].filter(Boolean).join(" ")}>
      {actual !== null && (
        <div className="volume-hero w-l">
          {speaker}
          <span className="volume-hero-val widget-figure">{animatedValue}%</span>
        </div>
      )}
      <div className="volume-bar-row">
        {actual === null && (
          <button type="button" className="volume-end" aria-label={t("volume.quieter")} onPointerDown={nudge(down)}>−</button>
        )}
        <div
          ref={track}
          className="volume-track"
          role="slider"
          aria-valuemin={0}
          aria-valuemax={100}
          aria-valuenow={value ?? 0}
          onPointerDown={onDown}
          onPointerMove={onMove}
          onPointerUp={onUp}
          onPointerCancel={onUp}
        >
          <div className="volume-fill" style={vertical ? { height: `${fill}%` } : { width: `${fill}%` }} />
          {!vertical && actual !== null && (
            <button
              type="button"
              className="volume-track-icon"
              aria-label={t("volume.mute")}
              onPointerDown={(e) => e.stopPropagation()}
              onClick={(e) => {
                e.stopPropagation();
                if (online) onPress(mute).catch(onPressError);
              }}
            >
              <Icon icon={icon} size={16} />
            </button>
          )}
          {actual !== null && <span className="volume-inside">{animatedValue}%</span>}
        </div>
        {actual === null && (
          <button type="button" className="volume-end" aria-label={t("volume.louder")} onPointerDown={nudge(up)}>+</button>
        )}
      </div>
    </div>
  );
}
