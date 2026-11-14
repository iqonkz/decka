import { useEffect, useState } from "react";
import type { ClockItem } from "@decka/shared";
import { formatClock, formatDate } from "./format";

export default function Clock({ item }: { item: ClockItem }) {
  const seconds = item.seconds === true;
  const wide = item.w >= 2;
  const [now, setNow] = useState(() => new Date());

  useEffect(() => {
    const timer = window.setInterval(() => setNow(new Date()), seconds ? 1000 : 10_000);
    return () => window.clearInterval(timer);
  }, [seconds]);

  return (
    <div className="clock">
      <div className="clock-time" style={{ fontSize: wide ? "9vmin" : "5.5vmin" }}>
        {formatClock(now, seconds)}
      </div>
      {wide && <div className="clock-date">{formatDate(now)}</div>}
    </div>
  );
}
