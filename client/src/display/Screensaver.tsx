import { useEffect, useRef, useState } from "react";
import { locale, t } from "@decka/shared";
import { useAppState } from "../store/store";
import { Icon } from "@decka/ui";
import { saverPosition } from "./saverPosition";

const MOVE_EVERY_MS = 30_000;
const FADE_MS = 1000;

const two = (n: number) => String(n).padStart(2, "0");
const hhmm = (d: Date) => `${two(d.getHours())}:${two(d.getMinutes())}`;
const formatDateLine = (d: Date) =>
  d.toLocaleDateString(locale(), { weekday: "short", day: "numeric", month: "long" });
const seedOf = (now: number) => Math.floor(now / MOVE_EVERY_MS);

/**
 * Near-black screen with large tabular-nums clock, date, and playing track that shifts every 30 s
 * against OLED burn-in (M31 T31.8b). First touch wakes the screen.
 */
export default function Screensaver() {
  const offline = useAppState((s) => s.conn === "offline");
  const errors = useAppState((s) => s.state?.statuses.filter((st) => st.state === "error").length ?? 0);
  const media = useAppState((s) => s.state?.media ?? null);
  const [now, setNow] = useState(() => new Date());
  const [seed, setSeed] = useState(() => seedOf(Date.now()));
  const [pos, setPos] = useState<{ x: number; y: number } | null>(null);
  const [visible, setVisible] = useState(true);
  const box = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const timer = window.setInterval(() => {
      const ts = Date.now();
      setNow(new Date(ts));
      setSeed(seedOf(ts));
    }, 1000);
    return () => window.clearInterval(timer);
  }, []);

  useEffect(() => {
    const place = () => {
      const el = box.current;
      const size = { w: el?.offsetWidth ?? 0, h: el?.offsetHeight ?? 0 };
      setPos(saverPosition(seed, { w: window.innerWidth, h: window.innerHeight }, size));
    };
    if (pos === null) {
      place();
      return;
    }
    setVisible(false);
    const timer = window.setTimeout(() => {
      place();
      setVisible(true);
    }, FADE_MS);
    return () => window.clearTimeout(timer);
    // `pos` is deliberately omitted: only interval transitions reposition the clock.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [seed]);

  // The track line is for music that plays; a paused player shows nothing.
  const trackLine = media?.playing && media.title
    ? media.artist
      ? `${media.artist} — ${media.title}`
      : media.title
    : null;

  return (
    <div className="saver" onPointerDown={(e) => e.stopPropagation()} onPointerUp={(e) => e.stopPropagation()}>
      <div
        ref={box}
        className="saver-box"
        style={{ left: pos?.x ?? 16, top: pos?.y ?? 16, opacity: visible && pos ? 1 : 0 }}
      >
        <div className="saver-time">{hhmm(now)}</div>
        <div className="saver-date">{formatDateLine(now)}</div>
        {trackLine && (
          <div className="saver-track">
            <Icon icon="lucide:music" size={14} />
            <span>{trackLine}</span>
          </div>
        )}
        {offline && <div className="saver-line">{t("saver.offline")}</div>}
        {errors > 0 && <div className="saver-line saver-errors">● {errors}</div>}
      </div>
    </div>
  );
}
