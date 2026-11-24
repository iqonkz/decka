import { useEffect, useState } from "react";
import type { ClockItem } from "@decka/shared";
import { formatClock, formatDate } from "./format";

export default function Clock({ item }: { item: ClockItem }) {
  const seconds = item.seconds === true;
  const [now, setNow] = useState(() => new Date());

  useEffect(() => {
    const timer = window.setInterval(() => setNow(new Date()), seconds ? 1000 : 10_000);
    return () => window.clearInterval(timer);
  }, [seconds]);

  return (
    <div className="clock">
      <div className="clock-time widget-figure">{formatClock(now, seconds)}</div>
      <div className="clock-date w-m">{formatDate(now)}</div>
    </div>
  );
}
