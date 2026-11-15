import { useEffect, useRef, useState } from "react";
import { t } from "@decka/shared";
import { useAppState } from "../store/store";
import { saverPosition } from "./saverPosition";

const MOVE_EVERY_MS = 30_000;
const FADE_MS = 1000;

const two = (n: number) => String(n).padStart(2, "0");
const hhmm = (d: Date) => `${two(d.getHours())}:${two(d.getMinutes())}`;
const seedOf = (now: number) => Math.floor(now / MOVE_EVERY_MS);

/**
 * Near-black screen with a grey clock that hops to a new place every 30 s (fade out, move, fade in).
 * It swallows touches: the first one only wakes the screen.
 */
export default function Screensaver() {
  const offline = useAppState((s) => s.conn === "offline");
  const errors = useAppState((s) => s.state?.statuses.filter((st) => st.state === "error").length ?? 0);
  const [time, setTime] = useState(() => hhmm(new Date()));
  const [seed, setSeed] = useState(() => seedOf(Date.now()));
  const [pos, setPos] = useState<{ x: number; y: number } | null>(null);
  const [visible, setVisible] = useState(true);
  const box = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const timer = window.setInterval(() => {
      setTime(hhmm(new Date()));
      setSeed(seedOf(Date.now()));
    }, 1000);
    return () => window.clearInterval(timer);
  }, []);

  // A new interval: fade out, jump while invisible, fade in. The very first placement is immediate.
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
    // `pos` is deliberately not a dependency: only a new interval moves the block.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [seed]);

  return (
    <div className="saver" onPointerDown={(e) => e.stopPropagation()} onPointerUp={(e) => e.stopPropagation()}>
      <div
        ref={box}
        className="saver-box"
        style={{ left: pos?.x ?? 16, top: pos?.y ?? 16, opacity: visible && pos ? 1 : 0 }}
      >
        <div className="saver-time">{time}</div>
        {offline && <div className="saver-line">{t("saver.offline")}</div>}
        {errors > 0 && <div className="saver-line saver-errors">● {errors}</div>}
      </div>
    </div>
  );
}
